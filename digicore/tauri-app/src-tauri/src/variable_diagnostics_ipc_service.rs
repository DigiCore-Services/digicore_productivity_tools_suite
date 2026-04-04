//! Bounded inbound service for variable-input and diagnostics/test helper RPC orchestration.

use super::*;

pub(crate) async fn get_pending_variable_input(_host: ApiImpl) -> Result<Option<PendingVarDto>, String> {
    let display = variable_input::get_viewport_modal_display();
    log::info!("[Api] get_pending_variable_input: has_display={}", display.is_some());
    if let Some((content, vars, values, choice_indices, checkbox_checked)) = display {
        log::info!(
            "[Api] get_pending_variable_input: content_len={}, vars_count={}",
            content.len(),
            vars.len()
        );
        Ok(Some(PendingVarDto {
            content,
            vars: vars
                .iter()
                .map(|v| InteractiveVarDto {
                    tag: v.tag.clone(),
                    label: v.label.clone(),
                    var_type: var_type_to_string(&v.var_type).to_string(),
                    options: v.options.clone(),
                })
                .collect(),
            values,
            choice_indices: choice_indices.into_iter().map(|(k, v)| (k, v as u32)).collect(),
            checkbox_checked,
        }))
    } else {
        Ok(None)
    }
}

pub(crate) async fn submit_variable_input(
    _host: ApiImpl,
    values: HashMap<String, String>,
) -> Result<(), String> {
    if let Some(state) = variable_input::take_viewport_modal() {
        let clip_history: Vec<String> = clipboard_history::get_entries()
            .iter()
            .map(|e| e.content.clone())
            .collect();
        let processed = template_processor::process_with_user_vars(
            &state.content,
            None,
            &clip_history,
            Some(&values),
        );
        let hwnd = state.target_hwnd;
        if let Some(ref tx) = state.response_tx {
            let _ = tx.send((Some(processed), hwnd));
        } else {
            digicore_text_expander::drivers::hotstring::request_expansion(processed);
        }
    }
    Ok(())
}

pub(crate) async fn cancel_variable_input(_host: ApiImpl) -> Result<(), String> {
    if let Some(state) = variable_input::take_viewport_modal() {
        if let Some(ref tx) = state.response_tx {
            let _ = tx.send((None, None));
        }
    }
    Ok(())
}

pub(crate) async fn get_expansion_stats(_host: ApiImpl) -> Result<ExpansionStatsDto, String> {
    let stats = expansion_stats::get_stats();
    Ok(ExpansionStatsDto {
        total_expansions: stats.total_expansions as u32,
        total_chars_saved: stats.total_chars_saved as u32,
        estimated_time_saved_secs: stats.estimated_time_saved_secs(),
        top_triggers: stats
            .top_triggers(10)
            .into_iter()
            .map(|(s, c)| (s, c as u32))
            .collect(),
    })
}

pub(crate) async fn reset_expansion_stats(_host: ApiImpl) -> Result<(), String> {
    expansion_stats::reset_stats();
    Ok(())
}

pub(crate) async fn get_diagnostic_logs(_host: ApiImpl) -> Result<Vec<DiagnosticEntryDto>, String> {
    let entries = expansion_diagnostics::get_recent();
    Ok(entries
        .into_iter()
        .map(|e| DiagnosticEntryDto {
            timestamp_ms: e.timestamp_ms as u32,
            level: e.level,
            message: e.message,
        })
        .collect())
}

pub(crate) async fn clear_diagnostic_logs(_host: ApiImpl) -> Result<(), String> {
    expansion_diagnostics::clear();
    Ok(())
}

pub(crate) async fn test_snippet_logic(
    _host: ApiImpl,
    content: String,
    user_values: Option<HashMap<String, String>>,
) -> Result<SnippetLogicTestResultDto, String> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return Ok(SnippetLogicTestResultDto {
            result: String::new(),
            requires_input: false,
            vars: Vec::new(),
        });
    }

    let vars = template_processor::collect_interactive_vars(trimmed);
    let vars_dto: Vec<InteractiveVarDto> = vars
        .iter()
        .map(|v| InteractiveVarDto {
            tag: v.tag.clone(),
            label: v.label.clone(),
            var_type: var_type_to_string(&v.var_type).to_string(),
            options: v.options.clone(),
        })
        .collect();

    if !vars_dto.is_empty() && user_values.is_none() {
        return Ok(SnippetLogicTestResultDto {
            result: String::new(),
            requires_input: true,
            vars: vars_dto,
        });
    }

    let current_clipboard = arboard::Clipboard::new()
        .ok()
        .and_then(|mut c| c.get_text().ok());
    let clip_history: Vec<String> = clipboard_history::get_entries()
        .into_iter()
        .map(|e| e.content)
        .collect();

    let result = template_processor::process_for_preview(
        trimmed,
        current_clipboard.as_deref(),
        &clip_history,
        user_values.as_ref(),
    );
    let requires_input = !vars_dto.is_empty() && user_values.is_none();

    Ok(SnippetLogicTestResultDto {
        result,
        requires_input,
        vars: vars_dto,
    })
}

pub(crate) async fn kms_evaluate_placeholders(
    host: ApiImpl,
    content: String,
    user_values: Option<HashMap<String, String>>,
) -> Result<SnippetLogicTestResultDto, String> {
    test_snippet_logic(host, content, user_values).await
}

