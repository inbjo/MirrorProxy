//! Merge only the source setting into recognized user configuration formats.

use anyhow::Context;
use toml_edit::{value, DocumentMut, Item, Table};

pub(super) fn merge(target: &str, original: &str, repo: &str) -> anyhow::Result<Option<String>> {
    let merged = match target {
        "go" => set_assignment(original, "GOPROXY", &format!("{repo},direct")),
        "npm" => set_assignment(original, "registry", repo),
        "pip" => set_pip_index(original, repo)?,
        "homebrew" => set_assignment(
            original,
            "HOMEBREW_BOTTLE_DOMAIN",
            repo.trim_end_matches('/'),
        ),
        "cargo" | "bun" | "pdm" => set_toml_source(target, original, repo)?,
        _ => return Ok(None),
    };
    Ok(Some(merged))
}

fn set_assignment(original: &str, key: &str, value: &str) -> String {
    let mut output = String::new();
    let mut replaced = false;
    let newline = if original.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    for line in original.split_inclusive('\n') {
        if line
            .split_once('=')
            .is_some_and(|(name, _)| name.trim() == key)
        {
            output.push_str(&format!("{key}={value}{newline}"));
            replaced = true;
        } else {
            output.push_str(line);
        }
    }
    if !replaced {
        if !output.is_empty() && !output.ends_with('\n') {
            output.push_str(newline);
        }
        output.push_str(&format!("{key}={value}{newline}"));
    }
    output
}

fn set_pip_index(original: &str, repo: &str) -> anyhow::Result<String> {
    let newline = if original.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let option = format!("index-url = {repo}{newline}");
    let mut output = String::new();
    let mut section = "";
    let mut saw_global = false;
    let mut global_option = false;
    let mut lines = original.lines().peekable();
    while let Some(line) = lines.next() {
        let trimmed = line.trim();
        if let Some(header) = trimmed
            .strip_prefix('[')
            .and_then(|rest| rest.split_once(']'))
        {
            if section == "global" && !global_option {
                output.push_str(&option);
            }
            section = header.0;
            if section == "global" {
                anyhow::ensure!(
                    !saw_global,
                    "duplicate [global] section in pip configuration"
                );
                saw_global = true;
            }
        } else if matches!(
            section,
            "global" | "install" | "download" | "wheel" | "index" | "list"
        ) && trimmed.split_once(['=', ':']).is_some_and(|(key, _)| {
            key.trim()
                .replace('_', "-")
                .eq_ignore_ascii_case("index-url")
        }) {
            let indentation = line.len() - line.trim_start().len();
            while lines.peek().is_some_and(|next| {
                let trimmed = next.trim_start();
                next.len() - trimmed.len() > indentation
                    && !trimmed.is_empty()
                    && !trimmed.starts_with(['#', ';'])
            }) {
                lines.next();
            }
            if section == "global" {
                anyhow::ensure!(
                    !global_option,
                    "duplicate index-url in pip [global] configuration"
                );
                global_option = true;
            }
            output.push_str(&line[..indentation]);
            output.push_str(&option);
            continue;
        }
        output.push_str(line);
        output.push_str(newline);
    }
    if !saw_global {
        output.push_str(&format!("[global]{newline}"));
        output.push_str(&option);
    } else if section == "global" && !global_option {
        output.push_str(&option);
    }
    Ok(output)
}

fn table<'a>(parent: &'a mut Table, key: &str) -> anyhow::Result<&'a mut Table> {
    parent
        .entry(key)
        .or_insert(Item::Table(Table::new()))
        .as_table_mut()
        .with_context(|| format!("expected [{key}] to be a standard TOML table"))
}

fn set_toml_source(target: &str, original: &str, repo: &str) -> anyhow::Result<String> {
    let mut document = original
        .parse::<DocumentMut>()
        .context("existing configuration is not valid TOML")?;
    let root = document.as_table_mut();
    match target {
        "cargo" => {
            let sources = table(root, "source")?;
            table(sources, "crates-io")?["replace-with"] = value("mirrorproxy");
            let mirror = table(sources, "mirrorproxy")?;
            anyhow::ensure!(
                !["git", "directory", "local-registry", "replace-with"]
                    .iter()
                    .any(|key| mirror.contains_key(key)),
                "Cargo source 'mirrorproxy' is already configured with a conflicting source"
            );
            mirror["registry"] = value(super::cargo_registry_url(repo));
        }
        "bun" => table(root, "install")?["registry"] = value(repo),
        "pdm" => table(root, "pypi")?["url"] = value(repo),
        _ => unreachable!(),
    }
    Ok(document.to_string())
}
