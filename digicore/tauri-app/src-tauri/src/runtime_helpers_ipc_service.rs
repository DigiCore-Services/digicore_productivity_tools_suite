//! Bounded inbound service for runtime helper RPC orchestration.

use super::*;

pub(crate) async fn get_running_process_names(_host: ApiImpl) -> Result<Vec<String>, String> {
    Ok(crate::appearance_enforcement::get_running_process_names())
}

pub(crate) async fn get_weather_location_suggestions(
    _host: ApiImpl,
    city_query: String,
    country: Option<String>,
    region: Option<String>,
) -> Result<Vec<String>, String> {
    let query = city_query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }
    let registry = digicore_text_expander::application::scripting::get_registry();
    digicore_text_expander::application::scripting::weather_location_suggestions(
        query,
        country.as_deref(),
        region.as_deref(),
        registry.http_fetcher.as_ref(),
    )
    .map(|v| v.into_iter().take(10).collect())
}

