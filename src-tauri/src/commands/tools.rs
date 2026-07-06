//! 内置工具注册与执行。
//!
//! 工具定义作为 `ToolDefinition` 注入 ChatRequest，AI 调用时经 dispatch 路由到执行器。

use ripple_core::{ToolDefinition, ToolSource};

/// 注册全部工具定义（内置 + 插件）
pub fn builtin_tools() -> Vec<ToolDefinition> {
    let mut tools: Vec<ToolDefinition> = vec![
        calculator_tool(),
        rag_search_tool(),
        get_time_info_tool(),
        get_weather_tool(),
        remember_tool(),
    ];
    // 扫描并加载插件工具
    let plugin_tools = crate::commands::plugins::plugin_tools();
    tools.extend(plugin_tools);
    tools
}

// ---- 记忆工具 ----

fn remember_tool() -> ToolDefinition {
    ToolDefinition {
        name: "remember".into(),
        description: "Store information to your long-term memory for future conversations. Use this when the user asks you to remember something, or when you learn important facts/preferences about the user. Always include full context: time, scenario, feelings, specific details (2-3 sentences).".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "content": {
                    "type": "string",
                    "description": "The information to remember (include rich context: time, scenario, feelings, details - 2-3 sentences)"
                }
            },
            "required": ["content"]
        }),
        source: ToolSource::Builtin,
        requires_approval: false,
    }
}

// ---- RAG 搜索 ----

fn rag_search_tool() -> ToolDefinition {
    ToolDefinition {
        name: "rag_search".into(),
        description: "Search the knowledge base for relevant information. Use this when the user asks questions about their documents or uploaded files.".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query to find relevant information"
                },
                "kb_id": {
                    "type": "string",
                    "description": "Optional knowledge base ID to search in. Omit to search all."
                },
                "top_k": {
                    "type": "integer",
                    "description": "Number of results to return (default 5)",
                    "default": 5
                }
            },
            "required": ["query"]
        }),
        source: ToolSource::Builtin,
        requires_approval: false,
    }
}

// ---- 计算器 ----

fn calculator_tool() -> ToolDefinition {
    ToolDefinition {
        name: "calculator".into(),
        description: "Evaluate a mathematical expression. Supports + - * / ^ sqrt() sin() cos() tan() log() ln() pi e and parentheses.".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "expression": {
                    "type": "string",
                    "description": "The mathematical expression to evaluate, e.g. '2 + 2 * 3' or 'sqrt(pi * 16)' "
                }
            },
            "required": ["expression"]
        }),
        source: ToolSource::Builtin,
        requires_approval: false,
    }
}

/// 执行计算器工具
pub fn exec_calculator(args: &serde_json::Value) -> Result<String, String> {
    let expr = args.get("expression")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "missing 'expression' argument".to_string())?;

    // 安全数学求值（使用 meval crate 或简单解析）
    // 这里用 meval 风格的简单解析：只允许数字、运算符、函数
    eval_math(expr)
}

/// 安全数学求值器（递归下降解析，支持 + - * / ^ 和括号）
fn eval_math(expr: &str) -> Result<String, String> {
    let chars: Vec<char> = expr.chars().filter(|c| !c.is_whitespace()).collect();
    let mut pos = 0;
    let result = parse_expr(&chars, &mut pos)?;
    if pos < chars.len() {
        return Err(format!("unexpected character '{}' at position {}", chars[pos], pos));
    }
    Ok(format_num(result))
}

fn parse_expr(chars: &[char], pos: &mut usize) -> Result<f64, String> {
    let mut left = parse_term(chars, pos)?;
    while *pos < chars.len() {
        match chars[*pos] {
            '+' => { *pos += 1; left += parse_term(chars, pos)?; }
            '-' => { *pos += 1; left -= parse_term(chars, pos)?; }
            _ => break,
        }
    }
    Ok(left)
}

fn parse_term(chars: &[char], pos: &mut usize) -> Result<f64, String> {
    let mut left = parse_power(chars, pos)?;
    while *pos < chars.len() {
        match chars[*pos] {
            '*' => { *pos += 1; left *= parse_power(chars, pos)?; }
            '/' => {
                *pos += 1;
                let right = parse_power(chars, pos)?;
                if right == 0.0 { return Err("division by zero".into()); }
                left /= right;
            }
            '%' => {
                *pos += 1;
                let right = parse_power(chars, pos)?;
                if right == 0.0 { return Err("modulo by zero".into()); }
                left %= right;
            }
            _ => break,
        }
    }
    Ok(left)
}

fn parse_power(chars: &[char], pos: &mut usize) -> Result<f64, String> {
    let mut left = parse_unary(chars, pos)?;
    if *pos < chars.len() && chars[*pos] == '^' {
        *pos += 1;
        let right = parse_power(chars, pos)?;  // 右结合
        left = left.powf(right);
    }
    Ok(left)
}

fn parse_unary(chars: &[char], pos: &mut usize) -> Result<f64, String> {
    if *pos >= chars.len() {
        return Err("unexpected end of expression".into());
    }
    match chars[*pos] {
        '+' => { *pos += 1; parse_primary(chars, pos) }
        '-' => { *pos += 1; Ok(-parse_primary(chars, pos)?) }
        _ => parse_primary(chars, pos),
    }
}

fn parse_primary(chars: &[char], pos: &mut usize) -> Result<f64, String> {
    if *pos >= chars.len() {
        return Err("unexpected end".into());
    }

    // 函数调用
    let funcs = ["sqrt", "sin", "cos", "tan", "log", "ln", "abs", "ceil", "floor", "round"];
    for &f in &funcs {
        let fc: Vec<char> = f.chars().collect();
        if chars[*pos..].starts_with(&fc) {
            *pos += f.len();
            if *pos >= chars.len() || chars[*pos] != '(' {
                return Err(format!("expected '(' after '{f}'"));
            }
            *pos += 1;
            let inner = parse_expr(chars, pos)?;
            if *pos >= chars.len() || chars[*pos] != ')' {
                return Err("expected ')'".into());
            }
            *pos += 1;
            return Ok(match f {
                "sqrt" => inner.sqrt(),
                "sin" => inner.sin(),
                "cos" => inner.cos(),
                "tan" => inner.tan(),
                "log" => inner.log10(),
                "ln" => inner.ln(),
                "abs" => inner.abs(),
                "ceil" => inner.ceil(),
                "floor" => inner.floor(),
                "round" => inner.round(),
                _ => unreachable!(),
            });
        }
    }

    // 常量
    if chars[*pos..].len() >= 2 && chars[*pos] == 'p' && chars[*pos + 1] == 'i'
        && (*pos + 2 >= chars.len() || !chars[*pos + 2].is_alphanumeric()) {
        *pos += 2;
        return Ok(std::f64::consts::PI);
    }
    if chars[*pos] == 'e' && (*pos + 1 >= chars.len() || !chars[*pos + 1].is_alphanumeric()) {
        *pos += 1;
        return Ok(std::f64::consts::E);
    }

    // 括号
    if chars[*pos] == '(' {
        *pos += 1;
        let inner = parse_expr(chars, pos)?;
        if *pos >= chars.len() || chars[*pos] != ')' {
            return Err("expected ')'".into());
        }
        *pos += 1;
        return Ok(inner);
    }

    // 数字
    parse_number(chars, pos)
}

fn parse_number(chars: &[char], pos: &mut usize) -> Result<f64, String> {
    let start = *pos;
    while *pos < chars.len() && (chars[*pos].is_ascii_digit() || chars[*pos] == '.') {
        *pos += 1;
    }
    if *pos == start {
        return Err(format!("expected number at position {start}, got '{:?}'", chars.get(start)));
    }
    let s: String = chars[start..*pos].iter().collect();
    s.parse::<f64>().map_err(|e| format!("invalid number '{s}': {e}"))
}

fn format_num(v: f64) -> String {
    if v.is_infinite() { return "Infinity".into(); }
    if v.is_nan() { return "NaN".into(); }
    let s = format!("{:.10}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    s.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_arithmetic() {
        assert_eq!(eval_math("2+3").unwrap(), "5");
        assert_eq!(eval_math("10 - 4").unwrap(), "6");
        assert_eq!(eval_math("3*4").unwrap(), "12");
        assert_eq!(eval_math("15/3").unwrap(), "5");
    }

    #[test]
    fn precedence() {
        assert_eq!(eval_math("2+3*4").unwrap(), "14");
        assert_eq!(eval_math("(2+3)*4").unwrap(), "20");
        assert_eq!(eval_math("2^3^2").unwrap(), "512"); // 右结合: 2^(3^2)=2^9=512
    }

    #[test]
    fn functions_and_constants() {
        assert!((eval_math("sqrt(16)").unwrap().parse::<f64>().unwrap() - 4.0).abs() < 1e-6);
        assert!((eval_math("sin(0)").unwrap().parse::<f64>().unwrap() - 0.0).abs() < 1e-6);
        assert!((eval_math("pi").unwrap().parse::<f64>().unwrap() - std::f64::consts::PI).abs() < 1e-6);
    }

    #[test]
    fn unary_minus() {
        assert_eq!(eval_math("-5+3").unwrap(), "-2");
        assert_eq!(eval_math("-(4+5)").unwrap(), "-9");
    }

    #[test]
    fn errors() {
        assert!(eval_math("1/0").is_err());
        assert!(eval_math("2+").is_err());
        assert!(eval_math("(3").is_err());
    }
}

// ---- 时间日期 ----

fn get_time_info_tool() -> ToolDefinition {
    ToolDefinition {
        name: "get_time_info".into(),
        description: "Get the current date, time, and timezone. No parameters needed.".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {}
        }),
        source: ToolSource::Builtin,
        requires_approval: false,
    }
}

pub fn exec_get_time_info(_args: &serde_json::Value) -> Result<String, String> {
    use chrono::Local;
    let now = Local::now();
    let weekdays = ["星期一", "星期二", "星期三", "星期四", "星期五", "星期六", "星期日"];
    let wd = weekdays[now.format("%w").to_string().parse::<usize>().unwrap_or(1)];
    Ok(format!(
        "当前时间：{}\n当前日期：{}年{}月{}日 {}\n时区：UTC{}\n时间戳：{}",
        now.format("%H:%M:%S"),
        now.format("%Y"),
        now.format("%m"),
        now.format("%d"),
        wd,
        now.format("%z"),
        now.timestamp(),
    ))
}

// ---- 天气 ----

fn get_weather_tool() -> ToolDefinition {
    ToolDefinition {
        name: "get_weather".into(),
        description: "查询城市当前天气。参数 city：城市名，如 Beijing / London / Tokyo / 上海".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "city": {
                    "type": "string",
                    "description": "城市名，如 Beijing、London、Tokyo，中文也可"
                }
            },
            "required": ["city"]
        }),
        source: ToolSource::Builtin,
        requires_approval: false,
    }
}

pub async fn exec_get_weather(args: &serde_json::Value) -> Result<String, String> {
    let city = args.get("city")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "missing 'city' argument".to_string())?;

    let url = format!("https://wttr.in/{}?format=%C+|+%t+|+Humidity:%h+|+Wind:%w&lang=zh", urlencoding(city));
    let resp = reqwest::get(&url)
        .await
        .map_err(|e| format!("weather request failed: {e}"))?
        .text()
        .await
        .map_err(|e| format!("read response failed: {e}"))?;

    if resp.contains("Unknown location") {
        return Err(format!("未找到城市：{city}"));
    }
    Ok(format!("{city} 天气：{resp}"))
}

fn urlencoding(s: &str) -> String {
    s.replace(' ', "+")
}

#[cfg(test)]
mod calc_tests {
    use super::*;

    #[test]
    fn calculator_tool_definition() {
        let t = calculator_tool();
        assert_eq!(t.name, "calculator");
        assert!(t.description.len() > 10);
    }
}
