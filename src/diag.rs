/// Renders Rustc-style compiler error diagnostics with line numbers, source snippets,
/// caret markers (`^^^`), and smart typo suggestions!
pub fn render_diagnostic(file_path: &str, src: &str, err_msg: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!("\x1b[1;31merror\x1b[0m\x1b[1m: {}\x1b[0m\n", err_msg));

    // Try to extract line:col from error message (e.g., "... at 12:5")
    if let Some((line_no, col_no)) = extract_line_col(err_msg) {
        let lines: Vec<&str> = src.lines().collect();
        if line_no >= 1 && line_no <= lines.len() {
            let line_str = lines[line_no - 1];
            let line_num_str = line_no.to_string();
            let pad = " ".repeat(line_num_str.len());

            out.push_str(&format!(
                "  \x1b[1;36m-->\x1b[0m {}:{}:{}\n",
                file_path, line_no, col_no
            ));
            out.push_str(&format!("   \x1b[1;36m{} |\x1b[0m\n", pad));
            out.push_str(&format!(
                "   \x1b[1;36m{} |\x1b[0m {}\n",
                line_num_str, line_str
            ));
            let caret_offset = col_no.saturating_sub(1);
            let caret_pad = " ".repeat(caret_offset);
            out.push_str(&format!(
                "   \x1b[1;36m{} |\x1b[0m {}\x1b[1;31m^^^\x1b[0m\n",
                pad, caret_pad
            ));
        }
    }

    // If undefined variable, suggest closest identifier from source
    if let Some(rest) = err_msg.strip_prefix("Undefined variable '") {
        if let Some(var_name) = rest.strip_suffix('\'') {
            if let Some(suggestion) = find_closest_ident(src, var_name) {
                out.push_str(&format!(
                    "   \x1b[1;32m= help:\x1b[0m Did you mean `\x1b[1;32m{}\x1b[0m`?\n",
                    suggestion
                ));
            }
        }
    }

    out
}

fn extract_line_col(msg: &str) -> Option<(usize, usize)> {
    let at_idx = msg.rfind(" at ")?;
    let coord = &msg[at_idx + 4..];
    let mut parts = coord.split(':');
    let line: usize = parts.next()?.trim().parse().ok()?;
    let col_part = parts.next()?;
    let col_digits: String = col_part.chars().take_while(|c| c.is_ascii_digit()).collect();
    let col: usize = col_digits.parse().ok()?;
    Some((line, col))
}

fn find_closest_ident(src: &str, target: &str) -> Option<String> {
    let mut best: Option<(usize, String)> = None;
    for token in src.split(|c: char| !c.is_ascii_alphanumeric() && c != '_') {
        if token.len() < 2 || token == target || token.chars().next()?.is_ascii_digit() {
            continue;
        }
        let dist = levenshtein(target, token);
        if dist <= 2 {
            if best.as_ref().map(|(d, _)| dist < *d).unwrap_or(true) {
                best = Some((dist, token.to_string()));
            }
        }
    }
    best.map(|(_, s)| s)
}

fn levenshtein(a: &str, b: &str) -> usize {
    let ac: Vec<char> = a.chars().collect();
    let bc: Vec<char> = b.chars().collect();
    let mut dp = vec![vec![0; bc.len() + 1]; ac.len() + 1];
    for i in 0..=ac.len() {
        dp[i][0] = i;
    }
    for j in 0..=bc.len() {
        dp[0][j] = j;
    }
    for i in 1..=ac.len() {
        for j in 1..=bc.len() {
            let cost = if ac[i - 1] == bc[j - 1] { 0 } else { 1 };
            dp[i][j] = (dp[i - 1][j] + 1)
                .min(dp[i][j - 1] + 1)
                .min(dp[i - 1][j - 1] + cost);
        }
    }
    dp[ac.len()][bc.len()]
}
