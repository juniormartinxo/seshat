use std::collections::BTreeMap;
use std::io::{self, Write};

pub fn info(message: impl AsRef<str>) {
    println!("{}", message.as_ref());
}

pub fn warning(message: impl AsRef<str>) {
    eprintln!("Aviso: {}", message.as_ref());
}

pub fn error(message: impl AsRef<str>) {
    eprintln!("{}", message.as_ref());
}

pub fn success(message: impl AsRef<str>) {
    println!("{}", message.as_ref());
}

pub fn section(message: impl AsRef<str>) {
    println!("\n{}", message.as_ref());
}

pub fn summary(title: &str, items: &BTreeMap<String, String>) {
    println!("{title}");
    for (key, value) in items {
        println!("  {key}: {value}");
    }
}

pub fn confirm(prompt: &str, default: bool) -> anyhow::Result<bool> {
    if !is_interactive() {
        return Ok(default);
    }

    let suffix = if default { "[Y/n]" } else { "[y/N]" };
    print!("{prompt} {suffix} ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let normalized = input.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return Ok(default);
    }

    Ok(matches!(normalized.as_str(), "y" | "yes" | "s" | "sim"))
}

pub fn prompt(prompt: &str, default: Option<&str>) -> anyhow::Result<String> {
    if !is_interactive() {
        return Ok(default.unwrap_or_default().to_string());
    }

    match default {
        Some(value) if !value.is_empty() => print!("{prompt} [{value}]: "),
        _ => print!("{prompt}: "),
    }
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let value = input.trim();
    if value.is_empty() {
        Ok(default.unwrap_or_default().to_string())
    } else {
        Ok(value.to_string())
    }
}

pub fn is_interactive() -> bool {
    std::io::IsTerminal::is_terminal(&io::stdin())
}
