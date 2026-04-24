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
