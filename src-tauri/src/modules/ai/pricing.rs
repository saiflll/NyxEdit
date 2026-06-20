pub fn model_price(model: &str) -> (f64, f64) {
    let m = model.to_lowercase();
    if m.contains("gpt-4o-mini") {
        (0.00015, 0.0006)
    } else if m.contains("gpt-4o") {
        (0.0025, 0.01)
    } else if m.contains("gpt-4") && !m.contains("turbo") {
        (0.03, 0.06)
    } else if m.contains("gpt-4-turbo") || m.contains("gpt-4-1106") {
        (0.01, 0.03)
    } else if m.contains("gpt-3.5-turbo") {
        (0.0005, 0.0015)
    } else if m.contains("claude-3-opus") || m.contains("claude-opus") {
        (0.015, 0.075)
    } else if m.contains("claude-3-sonnet") || m.contains("claude-sonnet") {
        (0.003, 0.015)
    } else if m.contains("claude-3-haiku") || m.contains("claude-haiku") {
        (0.00025, 0.00125)
    } else if m.contains("gemini-1.5-pro") || m.contains("gemini-pro") {
        (0.00125, 0.005)
    } else if m.contains("gemini-1.5-flash") || m.contains("gemini-flash") {
        (0.000075, 0.0003)
    } else if m.contains("gemini-2.0") || m.contains("gemini-2") {
        (0.0001, 0.0004)
    } else if m.contains("deepseek-chat") || m.contains("deepseek-v3") {
        (0.00014, 0.00028)
    } else if m.contains("deepseek-reasoner") || m.contains("deepseek-r1") {
        (0.00055, 0.00219)
    } else if m.contains("llama") || m.contains("mistral") || m.contains("mixtral") || m.contains("qwen") || m.contains("deepseek") {
        (0.0005, 0.0015)
    } else {
        (0.001, 0.002)
    }
}

pub fn model_capability_score(model: &str) -> i32 {
    let m = model.to_lowercase();
    if m.contains("ultra") || m.contains("opus") { return 100; }
    if m.contains("-pro") || m.contains("_pro") || m.ends_with("pro")
        || m.contains("large") || m.contains("-max") || m.contains("r1") { return 80; }
    if m.contains("plus") || m.contains("medium") || m.contains("sonnet")
        || m.contains("70b") || m.contains("72b") { return 60; }
    if m.contains("flash") || m.contains("-mini") || m.contains("_mini")
        || m.contains("8b") || m.contains("7b") { return 40; }
    if m.contains("nano") || m.contains("tiny") || m.contains("lite") { return 20; }
    let version_score = m
        .split(|c: char| !c.is_ascii_digit() && c != '.')
        .filter_map(|s| s.parse::<f32>().ok())
        .fold(0.0f32, f32::max);
    if version_score > 0.0 { return (version_score * 10.0) as i32; }
    50
}
