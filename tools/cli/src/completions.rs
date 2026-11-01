//! Completion candidates come from the public help, keeping the private runner
//! independent of a second CLI framework. Scripts never build the application.
use crate::{Result, args::HELP};

pub fn script(shell: &str) -> Result<String> {
    let commands = HELP
        .split("Commands:\n")
        .nth(1)
        .unwrap()
        .split("Options:\n")
        .next()
        .unwrap()
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .collect::<Vec<_>>();
    let options = HELP
        .lines()
        .filter(|line| line.starts_with("  --"))
        .filter_map(|line| line.split_whitespace().next())
        .chain(["--version", "-h", "-V", "-p", "-F"])
        .collect::<Vec<_>>();
    let words = commands
        .iter()
        .chain(&options)
        .copied()
        .collect::<Vec<_>>()
        .join(" ");
    Ok(match shell {
        "bash" => format!(
            r#"_glory_complete() {{
    local current="${{COMP_WORDS[COMP_CWORD]}}" previous="${{COMP_WORDS[COMP_CWORD-1]}}"
    local words='{words}'
    local word
    for word in "${{COMP_WORDS[@]:1:COMP_CWORD-1}}"; do
        if [[ "$word" == -- ]]; then COMPREPLY=(); return; fi
    done
    case "$previous" in
        --platform) words='web desktop server' ;;
        completions) words='bash zsh fish powershell' ;;
        --manifest-path|--cli-path|--build-root|--bindgen|--wasm-opt)
            COMPREPLY=()
            while IFS= read -r word; do COMPREPLY+=("$word"); done < <(compgen -f -- "$current")
            return ;;
        --target|--package|-p|--bin|--features|-F|--wasm-opt-version|--port) COMPREPLY=(); return ;;
    esac
    COMPREPLY=( $(compgen -W "$words" -- "$current") )
}}
complete -F _glory_complete glory bin/glory ./bin/glory
"#
        ),
        "zsh" => format!(
            r#"#compdef glory bin/glory ./bin/glory
_glory_complete() {{
    local -a candidates
    local word
    for word in "${{words[@]:1:CURRENT-2}}"; do [[ "$word" == -- ]] && return; done
    case "${{words[CURRENT-1]}}" in
        --platform) candidates=(web desktop server) ;;
        completions) candidates=(bash zsh fish powershell) ;;
        --manifest-path|--cli-path|--build-root|--bindgen|--wasm-opt) _files; return ;;
        --target|--package|-p|--bin|--features|-F|--wasm-opt-version|--port) return ;;
        *) candidates=({words}) ;;
    esac
    compadd -- "${{candidates[@]}}"
}}
compdef _glory_complete glory bin/glory ./bin/glory
"#
        ),
        "fish" => {
            let mut output = String::new();
            for executable in ["glory", "bin/glory", "./bin/glory"] {
                output.push_str(&format!(
                    "complete -c {executable} -n 'not contains -- -- (commandline -opc)' -f -a '{}'\n",
                    commands.join(" ")
                ));
                for option in &options {
                    if let Some(long) = option.strip_prefix("--") {
                        let extra = match long {
                            "platform" => " -r -f -a 'web desktop server'",
                            "manifest-path" | "cli-path" | "build-root" | "bindgen"
                            | "wasm-opt" => " -r -F",
                            "target" | "package" | "bin" | "features" | "wasm-opt-version"
                            | "port" => " -r -f",
                            _ => "",
                        };
                        output.push_str(&format!("complete -c {executable} -n 'not contains -- -- (commandline -opc)' -l {long}{extra}\n"));
                    }
                }
            }
            output
        }
        "powershell" => format!(
            r#"Register-ArgumentCompleter -Native -CommandName glory,glory.ps1,./bin/glory.ps1 -ScriptBlock {{
    param($wordToComplete, $commandAst, $cursorPosition)
    $elements = @($commandAst.CommandElements | Where-Object {{ $_.Extent.StartOffset -lt $cursorPosition }})
    if ($elements | Where-Object {{ $_.Extent.Text -eq '--' }}) {{ return }}
    $previous = if ($wordToComplete -eq '' -and $elements.Count -gt 0) {{ $elements[-1].Extent.Text }} elseif ($elements.Count -gt 1) {{ $elements[-2].Extent.Text }} else {{ '' }}
    $candidates = switch ($previous) {{
        '--platform' {{ 'web','desktop','server' }}
        'completions' {{ 'bash','zsh','fish','powershell' }}
        {{ $_ -in @('--target','--package','-p','--bin','--features','-F','--wasm-opt-version','--port','--manifest-path','--cli-path','--build-root','--bindgen','--wasm-opt') }} {{ return }}
        default {{ '{words}'.Split(' ') }}
    }}
    $candidates | Where-Object {{ $_.StartsWith($wordToComplete, [System.StringComparison]::OrdinalIgnoreCase) }} | ForEach-Object {{
        [System.Management.Automation.CompletionResult]::new($_, $_, 'ParameterValue', $_)
    }}
}}
"#
        ),
        _ => return Err("unsupported completion shell".into()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scripts_offer_current_commands_and_overrides() {
        for shell in ["bash", "zsh", "fish", "powershell"] {
            let script = script(shell).unwrap();
            for word in [
                "doctor",
                "completions",
                "wasm-opt-version",
                "no-default-features",
            ] {
                assert!(script.contains(word), "{shell}: {word}");
            }
        }
        assert!(script("unknown").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn bash_completes_platforms_and_stops_at_application_arguments() {
        let input = format!(
            "{}\nCOMP_WORDS=(./bin/glory serve --platform d); COMP_CWORD=3; _glory_complete; [[ ${{COMPREPLY[*]}} == desktop ]] || exit 1\nCOMP_WORDS=(./bin/glory run -- --port); COMP_CWORD=3; _glory_complete; [[ ${{#COMPREPLY[@]}} == 0 ]]",
            script("bash").unwrap()
        );
        assert!(
            std::process::Command::new("bash")
                .args(["-c", &input])
                .status()
                .unwrap()
                .success()
        );
    }
}
