//! Account-limit and conversation-context usage surfaces.

use lucidity_ui::UsageView;
use serde_json::Value;

/// Build the dual usage surface from a stored JSON snapshot (or empty).
pub fn usage_view_from_snapshot(json: Option<&Value>, conversation_title: Option<&str>) -> UsageView {
    let Some(value) = json else {
        return UsageView::default();
    };

    if value.get("status").and_then(|v| v.as_str()) == Some("refresh_requested") {
        return UsageView {
            account_label: "Account limits".into(),
            account_detail: "Refresh requested — waiting for provider data.".into(),
            context_label: conversation_title
                .map(|t| format!("Context · {t}"))
                .unwrap_or_else(|| "Conversation context".into()),
            context_detail: "No context percentage yet.".into(),
            provider_unavailable: true,
        };
    }

    let account = value
        .get("account")
        .cloned()
        .or_else(|| value.get("quota").cloned());
    let context = value
        .get("context")
        .cloned()
        .or_else(|| value.get("session").cloned());

    let account_detail = account
        .as_ref()
        .map(format_usage_node)
        .unwrap_or_else(|| "Provider did not expose account limits.".into());
    let context_detail = context
        .as_ref()
        .map(format_usage_node)
        .unwrap_or_else(|| "Provider did not expose conversation context.".into());

    let provider_unavailable = account.is_none() && context.is_none();

    UsageView {
        account_label: "Account limits".into(),
        account_detail,
        context_label: conversation_title
            .map(|t| format!("Context · {t}"))
            .unwrap_or_else(|| "Conversation context".into()),
        context_detail,
        provider_unavailable,
    }
}

fn format_usage_node(value: &Value) -> String {
    if let Some(text) = value.as_str() {
        return text.to_owned();
    }
    if let Some(percent) = value.get("percent").and_then(|v| v.as_f64()) {
        let label = value
            .get("label")
            .and_then(|v| v.as_str())
            .unwrap_or("used");
        return format!("{label}: {percent:.0}%");
    }
    if let Some(weekly) = value.get("weekly_percent").and_then(|v| v.as_f64()) {
        return format!("Weekly: {weekly:.0}%");
    }
    value.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn default_is_provider_unavailable() {
        let view = usage_view_from_snapshot(None, None);
        assert!(view.provider_unavailable);
    }

    #[test]
    fn splits_account_and_context() {
        let value = json!({
            "account": {"weekly_percent": 41.0},
            "context": {"percent": 64.0, "label": "session"}
        });
        let view = usage_view_from_snapshot(Some(&value), Some("refactor"));
        assert!(!view.provider_unavailable);
        assert!(view.account_detail.contains("41"));
        assert!(view.context_detail.contains("64"));
        assert!(view.context_label.contains("refactor"));
    }
}
