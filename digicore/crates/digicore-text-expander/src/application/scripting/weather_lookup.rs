//! Weather lookup helper for {weather:...} placeholder.
//!
//! Supported placeholder input:
//! - Positional: {weather:City|CountryOrCode|State|format}
//! - Keyed: {weather:city=Tokyo|country=JP|state=Tokyo|format=summary}
//!
//! format values:
//! - summary (default), temperature, windspeed, winddirection, weather_text,
//!   weathercode, is_day, time, json

use super::http_port::HttpFetcherPort;
use reqwest::Url;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const GEOCODE_BASE_URL: &str = "https://geocoding-api.open-meteo.com/v1/search";
const FORECAST_BASE_URL: &str = "https://api.open-meteo.com/v1/forecast";
const WEATHER_CACHE_TTL: Duration = Duration::from_secs(300);
const SUGGESTIONS_CACHE_TTL: Duration = Duration::from_secs(120);

#[derive(Clone, Debug)]
struct WeatherQuery {
    city: String,
    country: Option<String>,
    state: Option<String>,
    output: WeatherOutput,
}

#[derive(Clone, Debug)]
enum WeatherOutput {
    Summary,
    Temperature,
    Windspeed,
    Winddirection,
    WeatherText,
    WeatherCode,
    IsDay,
    Time,
    Json,
}

#[derive(Clone, Debug)]
struct WeatherResolved {
    location_name: String,
    country: String,
    country_code: String,
    state: String,
    temperature: f64,
    windspeed: f64,
    winddirection: f64,
    is_day: bool,
    weather_code: i32,
    weather_text: String,
    time: String,
    raw_json: String,
}

#[derive(Clone, Debug)]
struct WeatherCacheEntry {
    cached_at: Instant,
    data: WeatherResolved,
}

static WEATHER_CACHE: OnceLock<Mutex<HashMap<String, WeatherCacheEntry>>> = OnceLock::new();
static SUGGESTIONS_CACHE: OnceLock<Mutex<HashMap<String, SuggestionsCacheEntry>>> = OnceLock::new();

#[derive(Clone, Debug)]
struct SuggestionsCacheEntry {
    cached_at: Instant,
    values: Vec<String>,
}

fn cache_store() -> &'static Mutex<HashMap<String, WeatherCacheEntry>> {
    WEATHER_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn suggestions_cache_store() -> &'static Mutex<HashMap<String, SuggestionsCacheEntry>> {
    SUGGESTIONS_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn read_cached(cache_key: &str) -> Option<WeatherResolved> {
    let now = Instant::now();
    let mut guard = cache_store().lock().ok()?;
    guard.retain(|_, v| now.duration_since(v.cached_at) <= WEATHER_CACHE_TTL);
    guard.get(cache_key).map(|entry| entry.data.clone())
}

fn write_cached(cache_key: String, data: WeatherResolved) {
    if let Ok(mut guard) = cache_store().lock() {
        guard.insert(
            cache_key,
            WeatherCacheEntry {
                cached_at: Instant::now(),
                data,
            },
        );
    }
}

fn read_suggestions_cached(cache_key: &str) -> Option<Vec<String>> {
    let now = Instant::now();
    let mut guard = suggestions_cache_store().lock().ok()?;
    guard.retain(|_, v| now.duration_since(v.cached_at) <= SUGGESTIONS_CACHE_TTL);
    guard.get(cache_key).map(|entry| entry.values.clone())
}

fn write_suggestions_cached(cache_key: String, values: Vec<String>) {
    if let Ok(mut guard) = suggestions_cache_store().lock() {
        guard.insert(
            cache_key,
            SuggestionsCacheEntry {
                cached_at: Instant::now(),
                values,
            },
        );
    }
}

#[cfg(test)]
pub fn clear_weather_cache_for_tests() {
    if let Ok(mut guard) = cache_store().lock() {
        guard.clear();
    }
    if let Ok(mut guard) = suggestions_cache_store().lock() {
        guard.clear();
    }
}

#[derive(Deserialize)]
struct GeocodeResponse {
    results: Option<Vec<GeocodeResult>>,
}

#[derive(Clone, Deserialize)]
struct GeocodeResult {
    name: String,
    #[serde(default)]
    country: String,
    #[serde(default)]
    country_code: String,
    #[serde(default)]
    admin1: String,
    latitude: f64,
    longitude: f64,
}

#[derive(Deserialize)]
struct ForecastResponse {
    current_weather: Option<CurrentWeather>,
}

#[derive(Deserialize)]
struct CurrentWeather {
    time: String,
    temperature: f64,
    windspeed: f64,
    winddirection: f64,
    weathercode: i32,
    is_day: i32,
}

pub fn resolve_weather_placeholder(inner: &str, fetcher: &dyn HttpFetcherPort) -> String {
    let query = match WeatherQuery::parse(inner) {
        Ok(q) => q,
        Err(e) => return e,
    };
    let cache_key = query.cache_key();
    if let Some(cached) = read_cached(&cache_key) {
        return format_weather_output(&cached, &query.output);
    }

    let geocode_url = match build_geocode_url(&query.city) {
        Ok(url) => url,
        Err(e) => return format!("[Weather Error: {}]", e),
    };
    let geocode_body = fetcher.fetch(geocode_url.as_str(), None);
    if is_fetch_error(&geocode_body) {
        return geocode_body;
    }
    let selected = match select_geocode_candidate(&geocode_body, &query) {
        Ok(v) => v,
        Err(e) => return e,
    };

    let forecast_url = match build_forecast_url(selected.latitude, selected.longitude) {
        Ok(url) => url,
        Err(e) => return format!("[Weather Error: {}]", e),
    };
    let forecast_body = fetcher.fetch(forecast_url.as_str(), None);
    if is_fetch_error(&forecast_body) {
        return forecast_body;
    }
    let resolved = match parse_weather(&forecast_body, &selected) {
        Ok(v) => v,
        Err(e) => return e,
    };

    write_cached(cache_key, resolved.clone());
    format_weather_output(&resolved, &query.output)
}

pub fn location_suggestions(
    city_query: &str,
    country: Option<&str>,
    state: Option<&str>,
    fetcher: &dyn HttpFetcherPort,
) -> Result<Vec<String>, String> {
    let query = city_query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }
    let cache_key = format!(
        "{}|{}|{}",
        normalize(query),
        country.map(normalize).unwrap_or_default(),
        state.map(normalize).unwrap_or_default()
    );
    if let Some(cached) = read_suggestions_cached(&cache_key) {
        return Ok(cached);
    }
    let geocode_url = build_geocode_url(query).map_err(|e| format!("[Weather Error: {}]", e))?;
    let geocode_body = fetcher.fetch(geocode_url.as_str(), None);
    if is_fetch_error(&geocode_body) {
        return Err(geocode_body);
    }
    let parsed: GeocodeResponse = serde_json::from_str(&geocode_body)
        .map_err(|e| format!("[Weather Error: invalid geocode JSON: {}]", e))?;
    let mut results = parsed.results.unwrap_or_default();
    filter_candidates(
        &mut results,
        country.map(String::from).as_deref(),
        state.map(String::from).as_deref(),
    );
    if results.is_empty() {
        return Ok(Vec::new());
    }
    let city_norm = normalize(query);
    results.sort_by_key(|r| std::cmp::Reverse(candidate_score(r, &city_norm)));
    let mut out = Vec::new();
    for r in results {
        let mut line = r.name.clone();
        if !r.admin1.is_empty() {
            line.push_str(", ");
            line.push_str(&r.admin1);
        }
        if !r.country.is_empty() {
            line.push_str(", ");
            line.push_str(&r.country);
        } else if !r.country_code.is_empty() {
            line.push_str(", ");
            line.push_str(&r.country_code);
        }
        if !out.iter().any(|x: &String| x == &line) {
            out.push(line);
        }
    }
    write_suggestions_cached(cache_key, out.clone());
    Ok(out)
}

fn is_fetch_error(value: &str) -> bool {
    value.starts_with('[') && value.ends_with(']')
}

fn build_geocode_url(city: &str) -> Result<Url, String> {
    Url::parse_with_params(
        GEOCODE_BASE_URL,
        &[("name", city), ("count", "8"), ("language", "en"), ("format", "json")],
    )
    .map_err(|e| e.to_string())
}

fn build_forecast_url(latitude: f64, longitude: f64) -> Result<Url, String> {
    Url::parse_with_params(
        FORECAST_BASE_URL,
        &[
            ("latitude", latitude.to_string()),
            ("longitude", longitude.to_string()),
            ("current_weather", "true".to_string()),
        ],
    )
    .map_err(|e| e.to_string())
}

fn select_geocode_candidate(body: &str, query: &WeatherQuery) -> Result<GeocodeResult, String> {
    let parsed: GeocodeResponse =
        serde_json::from_str(body).map_err(|e| format!("[Weather Error: invalid geocode JSON: {}]", e))?;
    let mut results = parsed
        .results
        .ok_or_else(|| "[Weather Error: location not found]".to_string())?;
    if results.is_empty() {
        return Err("[Weather Error: location not found]".to_string());
    }

    filter_candidates(&mut results, query.country.as_deref(), query.state.as_deref());
    if results.is_empty() {
        return Err("[Weather Error: no location match for supplied city/country/state]".to_string());
    }

    let city_norm = normalize(&query.city);
    results.sort_by_key(|r| std::cmp::Reverse(candidate_score(r, &city_norm)));
    Ok(results[0].clone())
}

fn filter_candidates(results: &mut Vec<GeocodeResult>, country: Option<&str>, state: Option<&str>) {
    if let Some(country) = country {
        let country_norm = normalize(country);
        results.retain(|r| {
            normalize(&r.country) == country_norm || normalize(&r.country_code) == country_norm
        });
    }
    if let Some(state) = state {
        let state_norm = normalize(state);
        results.retain(|r| normalize(&r.admin1) == state_norm);
    }
}

fn candidate_score(result: &GeocodeResult, city_norm: &str) -> i32 {
    let name = normalize(&result.name);
    if name == city_norm {
        100
    } else if name.contains(city_norm) {
        80
    } else {
        0
    }
}

fn parse_weather(body: &str, selected: &GeocodeResult) -> Result<WeatherResolved, String> {
    let parsed: ForecastResponse =
        serde_json::from_str(body).map_err(|e| format!("[Weather Error: invalid forecast JSON: {}]", e))?;
    let current = parsed
        .current_weather
        .ok_or_else(|| "[Weather Error: current weather data not present]".to_string())?;

    Ok(WeatherResolved {
        location_name: selected.name.clone(),
        country: selected.country.clone(),
        country_code: selected.country_code.clone(),
        state: selected.admin1.clone(),
        temperature: current.temperature,
        windspeed: current.windspeed,
        winddirection: current.winddirection,
        is_day: current.is_day == 1,
        weather_code: current.weathercode,
        weather_text: weather_code_to_text(current.weathercode).to_string(),
        time: current.time,
        raw_json: body.to_string(),
    })
}

fn format_weather_output(resolved: &WeatherResolved, output: &WeatherOutput) -> String {
    match output {
        WeatherOutput::Summary => {
            let mut location = resolved.location_name.clone();
            if !resolved.state.is_empty() {
                location.push_str(", ");
                location.push_str(&resolved.state);
            }
            if !resolved.country.is_empty() {
                location.push_str(", ");
                location.push_str(&resolved.country);
            } else if !resolved.country_code.is_empty() {
                location.push_str(", ");
                location.push_str(&resolved.country_code);
            }
            let day_part = if resolved.is_day { "day" } else { "night" };
            format!(
                "{location}: {:.1}C, {}, wind {:.1} km/h at {:.0} deg ({})",
                resolved.temperature,
                resolved.weather_text,
                resolved.windspeed,
                resolved.winddirection,
                day_part
            )
        }
        WeatherOutput::Temperature => format!("{:.1}", resolved.temperature),
        WeatherOutput::Windspeed => format!("{:.1}", resolved.windspeed),
        WeatherOutput::Winddirection => format!("{:.0}", resolved.winddirection),
        WeatherOutput::WeatherText => resolved.weather_text.clone(),
        WeatherOutput::WeatherCode => resolved.weather_code.to_string(),
        WeatherOutput::IsDay => {
            if resolved.is_day {
                "1".to_string()
            } else {
                "0".to_string()
            }
        }
        WeatherOutput::Time => resolved.time.clone(),
        WeatherOutput::Json => resolved.raw_json.clone(),
    }
}

fn normalize(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn weather_code_to_text(code: i32) -> &'static str {
    match code {
        0 => "Clear sky",
        1 => "Mainly clear",
        2 => "Partly cloudy",
        3 => "Overcast",
        45 => "Fog",
        48 => "Depositing rime fog",
        51 => "Light drizzle",
        53 => "Moderate drizzle",
        55 => "Dense drizzle",
        56 => "Light freezing drizzle",
        57 => "Dense freezing drizzle",
        61 => "Slight rain",
        63 => "Moderate rain",
        65 => "Heavy rain",
        66 => "Light freezing rain",
        67 => "Heavy freezing rain",
        71 => "Slight snow fall",
        73 => "Moderate snow fall",
        75 => "Heavy snow fall",
        77 => "Snow grains",
        80 => "Slight rain showers",
        81 => "Moderate rain showers",
        82 => "Violent rain showers",
        85 => "Slight snow showers",
        86 => "Heavy snow showers",
        95 => "Thunderstorm",
        96 => "Thunderstorm with slight hail",
        99 => "Thunderstorm with heavy hail",
        _ => "Unknown conditions",
    }
}

impl WeatherQuery {
    fn parse(inner: &str) -> Result<Self, String> {
        let mut city = String::new();
        let mut country = String::new();
        let mut state = String::new();
        let mut output = String::new();

        let mut positional_index = 0usize;
        for token in inner.split('|').map(str::trim).filter(|t| !t.is_empty()) {
            if let Some((raw_key, raw_value)) = token.split_once('=') {
                let key = normalize(raw_key);
                let value = raw_value.trim();
                match key.as_str() {
                    "city" => city = value.to_string(),
                    "country" | "country_code" => country = value.to_string(),
                    "state" | "region" | "admin1" => state = value.to_string(),
                    "format" | "output" | "field" => output = value.to_string(),
                    _ => {}
                }
                continue;
            }

            match positional_index {
                0 => city = token.to_string(),
                1 => country = token.to_string(),
                2 => state = token.to_string(),
                3 => output = token.to_string(),
                _ => {}
            }
            positional_index += 1;
        }

        if city.trim().is_empty() {
            return Err("[Weather Error: city is required]".to_string());
        }

        Ok(Self {
            city,
            country: optional(country),
            state: optional(state),
            output: WeatherOutput::parse(optional(output).as_deref()),
        })
    }

    fn cache_key(&self) -> String {
        format!(
            "{}|{}|{}",
            normalize(&self.city),
            self.country.as_deref().map(normalize).unwrap_or_default(),
            self.state.as_deref().map(normalize).unwrap_or_default()
        )
    }
}

impl WeatherOutput {
    fn parse(raw: Option<&str>) -> Self {
        match raw.map(normalize).as_deref() {
            Some("temperature") | Some("temp") => Self::Temperature,
            Some("windspeed") | Some("wind_speed") => Self::Windspeed,
            Some("winddirection") | Some("wind_direction") => Self::Winddirection,
            Some("weather_text") | Some("weather") | Some("condition") => Self::WeatherText,
            Some("weathercode") | Some("weather_code") => Self::WeatherCode,
            Some("is_day") | Some("day") => Self::IsDay,
            Some("time") => Self::Time,
            Some("json") | Some("raw") => Self::Json,
            _ => Self::Summary,
        }
    }
}

fn optional(value: String) -> Option<String> {
    if value.trim().is_empty() {
        None
    } else {
        Some(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    struct TestFetcher {
        data: HashMap<String, String>,
        count: AtomicU32,
    }

    impl TestFetcher {
        fn new(data: HashMap<String, String>) -> Self {
            Self {
                data,
                count: AtomicU32::new(0),
            }
        }
    }

    impl HttpFetcherPort for TestFetcher {
        fn fetch(&self, url: &str, _json_path: Option<&str>) -> String {
            self.count.fetch_add(1, Ordering::SeqCst);
            self.data
                .get(url)
                .cloned()
                .unwrap_or_else(|| "[HTTP Error: no mock]".to_string())
        }
    }

    #[test]
    fn parses_keyed_and_positional_query() {
        let q = WeatherQuery::parse("London|GB|England|temperature").unwrap();
        assert_eq!(q.city, "London");
        assert_eq!(q.country.as_deref(), Some("GB"));
        assert_eq!(q.state.as_deref(), Some("England"));
        assert!(matches!(q.output, WeatherOutput::Temperature));

        let q2 = WeatherQuery::parse("city=Tokyo|country=JP|state=Tokyo|format=summary").unwrap();
        assert_eq!(q2.city, "Tokyo");
        assert_eq!(q2.country.as_deref(), Some("JP"));
        assert_eq!(q2.state.as_deref(), Some("Tokyo"));
        assert!(matches!(q2.output, WeatherOutput::Summary));
    }

    #[test]
    fn resolves_weather_and_uses_cache() {
        clear_weather_cache_for_tests();
        let geocode_url = build_geocode_url("London").unwrap().to_string();
        let forecast_url = build_forecast_url(51.5074, -0.1278).unwrap().to_string();

        let geocode_json = r#"{
          "results": [
            {
              "name": "London",
              "country": "United Kingdom",
              "country_code": "GB",
              "admin1": "England",
              "latitude": 51.5074,
              "longitude": -0.1278
            }
          ]
        }"#;
        let forecast_json = r#"{
          "current_weather": {
            "time": "2026-03-06T01:30",
            "temperature": 13.0,
            "windspeed": 1.4,
            "winddirection": 136,
            "is_day": 0,
            "weathercode": 3
          }
        }"#;

        let mut data = HashMap::new();
        data.insert(geocode_url, geocode_json.to_string());
        data.insert(forecast_url, forecast_json.to_string());
        let fetcher = TestFetcher::new(data);

        let out1 = resolve_weather_placeholder("London|GB|England|summary", &fetcher);
        assert!(out1.contains("London"));
        assert!(out1.contains("13.0C"));
        assert!(out1.contains("Overcast"));
        assert_eq!(fetcher.count.load(Ordering::SeqCst), 2);

        let out2 = resolve_weather_placeholder("city=London|country=GB|state=England|format=temperature", &fetcher);
        assert_eq!(out2, "13.0");
        assert_eq!(fetcher.count.load(Ordering::SeqCst), 2);
    }
}

