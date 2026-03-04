//! Unit, integration, edge, and negative tests for Ghost Follower (F48-F59).

use digicore_core::domain::entities::Snippet;
use serial_test::serial;
use digicore_text_expander::application::ghost_follower::{self, FollowerEdge, GhostFollowerConfig, MonitorAnchor};
use std::collections::HashMap;

fn make_library_with_pinned() -> HashMap<String, Vec<Snippet>> {
    let mut lib = HashMap::new();
    lib.insert(
        "Cat1".to_string(),
        vec![
            Snippet::new("hi", "Hello"),
            Snippet::new("sig", "Best regards"),
        ],
    );
    let mut pinned1 = Snippet::new("addr", "123 Main St");
    pinned1.pinned = "true".to_string();
    let mut pinned2 = Snippet::new("email", "me@example.com");
    pinned2.pinned = "true".to_string();
    lib.insert("Cat2".to_string(), vec![pinned1, pinned2]);
    lib
}

#[test]
#[serial]
fn test_start_stop() {
    ghost_follower::stop();
    assert!(!ghost_follower::is_enabled());

    let config = GhostFollowerConfig::default();
    ghost_follower::start(config, HashMap::new());
    assert!(ghost_follower::is_enabled());

    ghost_follower::stop();
    assert!(!ghost_follower::is_enabled());
}

#[test]
#[serial]
fn test_get_pinned_snippets_empty_filter() {
    ghost_follower::stop();
    let config = GhostFollowerConfig::default();
    ghost_follower::start(config, make_library_with_pinned());

    let pinned = ghost_follower::get_pinned_snippets("");
    assert_eq!(pinned.len(), 2);
    let triggers: Vec<_> = pinned.iter().map(|(s, _, _)| s.trigger.as_str()).collect();
    assert!(triggers.contains(&"addr"));
    assert!(triggers.contains(&"email"));

    ghost_follower::stop();
}

#[test]
#[serial]
fn test_get_pinned_snippets_filter_by_trigger() {
    ghost_follower::stop();
    let config = GhostFollowerConfig::default();
    ghost_follower::start(config, make_library_with_pinned());

    let pinned = ghost_follower::get_pinned_snippets("addr");
    assert_eq!(pinned.len(), 1);
    assert_eq!(pinned[0].0.trigger, "addr");

    ghost_follower::stop();
}

#[test]
#[serial]
fn test_get_pinned_snippets_filter_by_content() {
    ghost_follower::stop();
    let config = GhostFollowerConfig::default();
    ghost_follower::start(config, make_library_with_pinned());

    let pinned = ghost_follower::get_pinned_snippets("Main");
    assert_eq!(pinned.len(), 1);
    assert!(pinned[0].0.content.contains("Main"));

    ghost_follower::stop();
}

#[test]
#[serial]
fn test_get_pinned_snippets_filter_by_category() {
    ghost_follower::stop();
    let config = GhostFollowerConfig::default();
    ghost_follower::start(config, make_library_with_pinned());

    let pinned = ghost_follower::get_pinned_snippets("Cat2");
    assert_eq!(pinned.len(), 2);

    ghost_follower::stop();
}

#[test]
#[serial]
fn test_get_pinned_snippets_no_match() {
    ghost_follower::stop();
    let config = GhostFollowerConfig::default();
    ghost_follower::start(config, make_library_with_pinned());

    let pinned = ghost_follower::get_pinned_snippets("nonexistent");
    assert!(pinned.is_empty());

    ghost_follower::stop();
}

#[test]
#[serial]
fn test_update_library_refreshes_pinned() {
    ghost_follower::stop();
    let config = GhostFollowerConfig::default();
    let mut lib = HashMap::new();
    let mut pinned = Snippet::new("old", "Old");
    pinned.pinned = "true".to_string();
    lib.insert("Cat".to_string(), vec![pinned]);
    ghost_follower::start(config, lib);

    let pinned = ghost_follower::get_pinned_snippets("");
    assert_eq!(pinned.len(), 1);
    assert_eq!(pinned[0].0.trigger, "old");

    let mut new_lib = HashMap::new();
    let mut new_pinned = Snippet::new("new", "New");
    new_pinned.pinned = "true".to_string();
    new_lib.insert("Cat".to_string(), vec![new_pinned]);
    ghost_follower::update_library(new_lib);

    let pinned = ghost_follower::get_pinned_snippets("");
    assert_eq!(pinned.len(), 1);
    assert_eq!(pinned[0].0.trigger, "new");

    ghost_follower::stop();
}

#[test]
#[serial]
fn test_set_get_search_filter() {
    ghost_follower::stop();
    let config = GhostFollowerConfig::default();
    ghost_follower::start(config, HashMap::new());

    ghost_follower::set_search_filter("test");
    assert_eq!(ghost_follower::get_search_filter(), "test");

    ghost_follower::stop();
}

#[test]
fn test_config_edge() {
    let config = GhostFollowerConfig {
        enabled: true,
        edge: FollowerEdge::Left,
        monitor_anchor: MonitorAnchor::Primary,
        search_filter: "".to_string(),
        hover_preview: true,
        collapse_delay_secs: 10,
    };
    assert_eq!(config.edge, FollowerEdge::Left);
}
