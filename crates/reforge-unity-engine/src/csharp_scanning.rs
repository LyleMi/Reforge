fn scan_csharp(root: &Path, config: &Config, scan: &mut Scan) -> Result<()> {
    for path in scan
        .paths
        .clone()
        .iter()
        .filter(|path| extension(path) == Some("cs"))
    {
        scan_csharp_file(root, config, scan, path)?;
    }
    Ok(())
}

fn scan_csharp_file(root: &Path, config: &Config, scan: &mut Scan, path: &Path) -> Result<()> {
    let source = fs::read_to_string(path)?;
    let display_path = display(root, path);
    let runtime = !display_path
        .split('/')
        .any(|part| part.eq_ignore_ascii_case("editor"));
    if runtime {
        record_editor_api_usage(scan, &display_path, &source);
    }
    if source.contains("MonoBehaviour") || source.contains("ScriptableObject") {
        record_unity_type_signals(scan, config, &display_path, &source);
        record_expensive_frame_calls(scan, &display_path, &source);
    }
    Ok(())
}

fn record_editor_api_usage(scan: &mut Scan, path: &str, source: &str) {
    for (line, text) in source.lines().enumerate() {
        if text.contains("UnityEditor.") || text.trim_start().starts_with("using UnityEditor") {
            push!(
                scan,
                "editor_api_in_runtime",
                path,
                line + 1,
                "UnityEditor API is reachable from runtime code",
                1,
                1,
            );
        }
    }
}

fn record_unity_type_signals(scan: &mut Scan, config: &Config, path: &str, source: &str) {
    let symbol = class_name(source).unwrap_or("Unity type");
    let fields = serialized_field_count(source);
    if fields > config.max_serialized_fields {
        push!(
            scan,
            "serialized_field_bloat",
            path,
            1,
            &format!("Unity type {symbol} has {fields} serialized fields"),
            fields,
            config.max_serialized_fields,
        );
    }
    let lifecycle = LIFECYCLE_METHODS
        .iter()
        .filter(|name| source.contains(&format!(" {name}(")))
        .count();
    if lifecycle > config.max_lifecycle_methods {
        push!(
            scan,
            "lifecycle_overload",
            path,
            1,
            &format!("Unity type {symbol} implements {lifecycle} lifecycle methods"),
            lifecycle,
            config.max_lifecycle_methods,
        );
    }
    record_subscription_balance(scan, path, source);
}

fn record_subscription_balance(scan: &mut Scan, path: &str, source: &str) {
    let subscriptions = source.matches("+=").count();
    let unsubscriptions = source.matches("-=").count();
    if subscriptions > unsubscriptions {
        push!(
            scan,
            "unbalanced_event_subscription",
            path,
            1,
            &format!(
                "Unity type has {subscriptions} subscriptions but {unsubscriptions} unsubscriptions"
            ),
            subscriptions,
            unsubscriptions.max(1),
        );
    }
}

fn record_expensive_frame_calls(scan: &mut Scan, path: &str, source: &str) {
    let mut prefix_bytes = 0;
    for (line, text) in source.lines().enumerate() {
        prefix_bytes += text.len() + 1;
        if is_expensive_call(text) && is_after_frame_method(&source[..prefix_bytes.min(source.len())])
        {
            push!(
                scan,
                "expensive_frame_call",
                path,
                line + 1,
                "Unity frame-loop path performs a repeated object or resource lookup",
                1,
                1,
            );
        }
    }
}

fn is_expensive_call(line: &str) -> bool {
    [
        "GameObject.Find",
        "FindObjectOfType",
        "FindFirstObjectByType",
        "Resources.Load",
        "GetComponent",
    ]
    .iter()
    .any(|call| line.contains(call))
}

fn is_after_frame_method(prefix: &str) -> bool {
    ["Update(", "FixedUpdate(", "LateUpdate("]
        .iter()
        .any(|method| prefix.contains(method))
}
