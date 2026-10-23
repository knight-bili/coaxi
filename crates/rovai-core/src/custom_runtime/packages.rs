use super::*;
use std::{fs,path::{Path,PathBuf},sync::Arc,time::Duration};
use tokio::process::Command;
use tokio_util::sync::CancellationToken;
pub(crate) type Log=Arc<tokio::sync::Mutex<Vec<u8>>>;
pub(crate) struct PackageEntry{pub executable:PathBuf,pub arguments:Vec<String>,pub package:Option<String>,pub version:String}
pub(crate) fn program(name:&str)->Result<PathBuf>{crate::runtime_discovery::resolve_active_command_path(name).context("custom_runtime.required_program_missing")}
pub(crate) fn command(executable:impl AsRef<std::ffi::OsStr>)->Command{let mut value=Command::new(executable);crate::runtime_discovery::configure_active_runtime_command(&mut value);value}
async fn execute(value:Command,log:Log,cancel:CancellationToken)->Result<()>{crate::managed_operation::run_bounded_command(value,"custom-acp-install",Duration::from_secs(300),cancel,log).await}
async fn required_version(executable:&Path,requirement:Option<&str>)->Result<()>{
    let Some(requirement)=requirement else{return Ok(())};let log=Log::default();let mut value=command(executable);value.arg("--version");execute(value,log.clone(),CancellationToken::new()).await?;
    let output=String::from_utf8_lossy(&log.lock().await).into_owned();let pattern=regex::Regex::new(r"\d+\.\d+\.\d+(?:-[A-Za-z0-9.-]+)?")?;let observed=pattern.find(&output).context("custom_runtime.required_version_unreadable")?;
    ensure!(semver::VersionReq::parse(requirement).context("custom_runtime.required_version_invalid")?.matches(&semver::Version::parse(observed.as_str())?),"custom_runtime.required_version_mismatch");Ok(())
}
fn arguments(spec:&Value)->Result<Vec<String>>{spec["args"].as_array().context("custom_runtime.arguments_invalid")?.iter().map(|v|v.as_str().map(str::to_owned).context("custom_runtime.arguments_invalid")).collect()}
fn child_file(root:&Path,relative:&str)->Result<PathBuf>{let path=root.join(super::artifacts::relative(relative)?).canonicalize()?;ensure!(path.starts_with(root.canonicalize()?)&&path.is_file(),"custom_runtime.package_entry_invalid");Ok(path)}
fn resolve_npx_bin<'a>(bin:&'a Value,package:&str,command:Option<&str>)->Result<&'a str>{
    let short_name=package.rsplit('/').next().unwrap_or(package);
    if let Some(path)=bin.as_str(){ensure!(command.map_or(true,|command|command==short_name),"custom_runtime.npm_command_missing");return Ok(path)}
    let commands=bin.as_object().context("custom_runtime.npm_command_missing")?;
    let selected=if let Some(command)=command{command}else if commands.contains_key(short_name){short_name}else if commands.len()==1{commands.keys().next().context("custom_runtime.npm_command_missing")?}else{anyhow::bail!("custom_runtime.npm_command_missing")};
    commands.get(selected).and_then(Value::as_str).context("custom_runtime.npm_command_missing")
}
pub(crate) async fn install_npx(root:&Path,spec:&Value,log:Log,cancel:CancellationToken)->Result<PackageEntry>{
    let node=program("node")?;required_version(&node,spec["nodeRequired"].as_str()).await?;
    // 调用 npm 的 JavaScript 入口，避免 Windows 的 npm.cmd 另过一层 shell。
    let npm_candidates=[node.parent().context("custom_runtime.node_parent_missing")?.join("node_modules/npm/bin/npm-cli.js"),node.parent().unwrap().join("../lib/node_modules/npm/bin/npm-cli.js")];
    let npm=npm_candidates.into_iter().find(|path|path.is_file()).context("custom_runtime.npm_entry_missing")?.canonicalize()?;
    let mut install=command(&node);install.arg(&npm).args(["install","--no-audit","--no-fund","--package-lock=true","--prefix"]).arg(root).arg("--").arg(text(spec,"package")?).current_dir(root);execute(install,log,cancel).await?;
    let lock:Value=serde_json::from_slice(&fs::read(root.join("package-lock.json"))?)?;let dependencies=lock["packages"][""]["dependencies"].as_object().context("custom_runtime.npm_lock_missing")?;
    ensure!(dependencies.len()==1,"custom_runtime.npm_root_ambiguous");let package=dependencies.keys().next().context("custom_runtime.npm_package_missing")?;
    let directory=child_directory(&root.join("node_modules"),package)?;let metadata:Value=serde_json::from_slice(&fs::read(directory.join("package.json"))?)?;
    let version=text(&metadata,"version")?.to_owned();ensure!(lock["packages"][format!("node_modules/{package}")]["version"].as_str()==Some(version.as_str()),"custom_runtime.npm_lock_version_mismatch");
    let relative=resolve_npx_bin(&metadata["bin"],package,spec["cmd"].as_str())?;
    let script=child_file(&directory,relative)?;let mut args=vec![script.to_string_lossy().into_owned()];args.extend(arguments(spec)?);
    Ok(PackageEntry{executable:node,arguments:args,package:Some(package.clone()),version})
}
fn child_directory(root:&Path,relative:&str)->Result<PathBuf>{let path=root.join(super::artifacts::relative(relative)?).canonicalize()?;ensure!(path.starts_with(root.canonicalize()?)&&path.is_dir(),"custom_runtime.package_directory_invalid");Ok(path)}
pub(crate) async fn install_uvx(root:&Path,spec:&Value,log:Log,cancel:CancellationToken)->Result<PackageEntry>{
    let uv=program("uv")?;required_version(&uv,spec["uvRequired"].as_str()).await?;let tools=root.join("tools");let bins=root.join("bin");fs::create_dir_all(&tools)?;fs::create_dir_all(&bins)?;
    let mut install=command(&uv);install.args(["tool","install"]);if let Some(python)=spec["python"].as_str(){install.arg("--python").arg(python);}install.arg("--").arg(text(spec,"package")?).env("UV_TOOL_DIR",&tools).env("UV_TOOL_BIN_DIR",&bins).env("UV_PYTHON_INSTALL_DIR",root.join("python")).current_dir(root);execute(install,log,cancel).await?;
    let directories=fs::read_dir(&tools)?.collect::<std::io::Result<Vec<_>>>()?;let directories:Vec<_>=directories.into_iter().filter(|entry|entry.file_type().is_ok_and(|kind|kind.is_dir())).collect();ensure!(directories.len()==1,"custom_runtime.uv_tool_ambiguous");
    let directory=directories[0].path();let receipt=fs::read_to_string(directory.join("uv-receipt.toml"))?;let parsed:toml::Value=toml::from_str(&receipt)?;
    let package=parsed.get("tool").and_then(|v|v.get("requirements")).and_then(toml::Value::as_array).and_then(|v|v.first()).and_then(|v|v.get("name")).and_then(toml::Value::as_str).context("custom_runtime.uv_receipt_missing")?.to_owned();
    let selected=spec["cmd"].as_str().unwrap_or(&package);ensure!(Path::new(selected).components().count()==1&&!selected.contains(['/', '\\',':']),"custom_runtime.uv_command_invalid");
    let entry=if cfg!(windows){directory.join("Scripts").join(format!("{selected}.exe"))}else{directory.join("bin").join(selected)};ensure!(entry.is_file(),"custom_runtime.uv_command_missing");
    let version=python_distribution_version(&directory,&package)?;
    Ok(PackageEntry{executable:entry.canonicalize()?,arguments:arguments(spec)?,package:Some(package),version})
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn npx_bin_selection_prefers_explicit_then_package_name_then_unique_bin() {
        let bins=json!({"run":"bin/run.js","widget":"bin/widget.js"});
        assert_eq!(resolve_npx_bin(&bins,"@scope/widget",Some("run")).unwrap(),"bin/run.js");
        assert_eq!(resolve_npx_bin(&bins,"@scope/widget",None).unwrap(),"bin/widget.js");
        assert_eq!(resolve_npx_bin(&json!({"other":"bin/other.js"}),"@scope/widget",None).unwrap(),"bin/other.js");
    }

    #[test]
    fn npx_bin_selection_rejects_missing_explicit_and_ambiguous_commands() {
        let bins=json!({"widget":"bin/widget.js","other":"bin/other.js"});
        assert!(resolve_npx_bin(&bins,"@scope/widget",Some("missing")).is_err());
        assert!(resolve_npx_bin(&bins,"@scope/another",None).is_err());
        assert!(resolve_npx_bin(&json!({"widget":"bin/widget.js"}),"@scope/widget",Some("missing")).is_err());
    }

    #[test]
    fn version_probe_argv_preserves_windows_paths_and_empty_arguments() {
        assert_eq!(words(r#""C:\new folder\tool.exe" --version """#).unwrap(),
            vec![r"C:\new folder\tool.exe", "--version", ""]);
        assert_eq!(words(r#"'C:\new folder\tool.exe' 'literal\path'"#).unwrap(),
            vec![r"C:\new folder\tool.exe", r"literal\path"]);
        assert_eq!(words(r#"tool "quote\"inside" "tail\\""#).unwrap(),
            vec!["tool", "quote\"inside", "tail\\"]);
        assert!(words(r#"tool "unclosed"#).is_err());
        assert!(words(" ").is_err());
        assert!(words(&vec!["arg"; 129].join(" ")).is_err());
    }
}
fn python_distribution_version(root:&Path,package:&str)->Result<String>{
    let normalize=|text:&str|text.to_ascii_lowercase().replace(['_','.'],"-");let target=normalize(package);let mut directories=vec![(root.to_path_buf(),0usize)];let mut visited=0;
    while let Some((directory,depth))=directories.pop(){visited+=1;ensure!(visited<=20_000,"custom_runtime.python_metadata_capacity");if depth>6{continue}for entry in fs::read_dir(directory)?{let entry=entry?;if !entry.file_type()?.is_dir(){continue}let path=entry.path();if path.extension().is_some_and(|v|v=="dist-info"){let metadata=path.join("METADATA");if fs::metadata(&metadata)?.len()>2*1024*1024{continue}let body=fs::read_to_string(metadata)?;let name=body.lines().find_map(|v|v.strip_prefix("Name: "));let version=body.lines().find_map(|v|v.strip_prefix("Version: "));if name.is_some_and(|v|normalize(v)==target){return version.map(str::to_owned).context("custom_runtime.python_version_missing")}}else{directories.push((path,depth+1))}}}
    anyhow::bail!("custom_runtime.python_version_missing")
}
/// 仅把显式版本探测拆成 argv；不交给 cmd、PowerShell 或 sh 解释。
/// 双引号内仅按连续反斜杠后接引号的规则转义，普通 Windows 路径反斜杠原样保留。
pub(crate) fn words(input:&str)->Result<Vec<String>>{
    let mut result=Vec::new();let mut value=String::new();let mut quote=None;let mut active=false;let mut characters=input.chars().peekable();
    while let Some(character)=characters.next(){
        if character=='\\'&&quote==Some('"'){
            let mut slashes=1;while characters.peek().copied()==Some('\\'){characters.next();slashes+=1}
            if characters.peek().copied()==Some('"'){
                characters.next();for _ in 0..slashes/2{value.push('\\')}
                if slashes%2==1{value.push('"')}else{quote=None}
            }else{for _ in 0..slashes{value.push('\\')}}
            active=true;continue
        }
        if let Some(expected)=quote{if character==expected{quote=None}else{value.push(character)}active=true}else if character=='\''||character=='"'{quote=Some(character);active=true}else if character.is_whitespace(){if active{result.push(std::mem::take(&mut value));active=false}}else{value.push(character);active=true}
    }
    ensure!(quote.is_none(),"custom_runtime.version_probe_quotes_invalid");if active{result.push(value)}ensure!(!result.is_empty()&&result.len()<=128,"custom_runtime.version_probe_invalid");Ok(result)
}
