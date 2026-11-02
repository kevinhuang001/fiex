use super::super::{decode, encode, entry, is_executable_target, URI};
use crate::core::files::Entry;
use std::path::{Path, PathBuf};

#[cfg(target_os = "windows")]
#[derive(serde::Deserialize)]
struct WindowsApp {
    name: String,
    #[serde(default)]
    aumid: String,
    #[serde(default)]
    path: String,
    #[serde(default)]
    icon: String,
}

#[cfg(target_os = "windows")]
pub(in crate::core::volumes::applications) fn resolve_windows_executable(
    value: &str,
) -> Option<String> {
    if !is_executable_target(value) {
        return None;
    }
    let path = if value.starts_with('{') {
        // AppsFolder returns Known Folder IDs for tools such as msinfo32.exe.
        // An unresolved GUID must never be passed to ShellExecute as a path.
        crate::core::volumes::windows_shell::resolve_known_folder_path(value)?
    } else {
        PathBuf::from(value)
    };
    // Reject stale Start Menu shortcuts and unresolvable shell identities
    // before they enter the VFS listing and its persistent directory cache.
    if !path.is_file() {
        return None;
    }
    Some(path.to_string_lossy().into_owned())
}

#[cfg(target_os = "windows")]
fn looks_like_path(value: &str) -> bool {
    value.len() > 2
        && (value.as_bytes()[1] == b':' || value.starts_with("\\\\") || value.starts_with('{'))
}

#[cfg(target_os = "windows")]
pub(in crate::core::volumes::applications) fn platform_list() -> Result<Vec<Entry>, String> {
    let script = r#"$ErrorActionPreference='SilentlyContinue'
$packages=@{}
Get-AppxPackage | ForEach-Object { $packages[$_.PackageFamilyName]=$_.InstallLocation }
$folder=(New-Object -ComObject Shell.Application).Namespace('shell:AppsFolder')
$items=@()
foreach($item in $folder.Items()){
  $aumid=[string]$item.ExtendedProperty('System.AppUserModel.ID')
  $path=[string]$item.Path
  $isUwp=[bool]($aumid -and $aumid -notmatch '[:\\]' -and $aumid -match '!')
  # shell:AppsFolder also contains documents, website shortcuts and folders from
  # the Start Menu. Keep executable targets only; uninstallers are executables
  # and stay visible on purpose. Packaged apps have no path and are kept as-is.
  if(-not $isUwp){
    if($path -match '^[a-zA-Z][a-zA-Z0-9+.-]*://'){continue}
    $target=[string]$item.ExtendedProperty('System.Link.TargetExtension')
    if(-not $target){$target=[IO.Path]::GetExtension($path)}
    $target=([string]$target).ToLowerInvariant()
    if(@('.exe','.com','.bat','.cmd','.msc','.cpl','.scr','.ps1','.appref-ms','.jar') -notcontains $target){continue}
  }
  $icon=''
  if($isUwp){
    $pfn=$aumid.Split('!')[0]
    $location=$packages[$pfn]
    if($location){
      $manifest=Join-Path $location 'AppxManifest.xml'
      if(Test-Path $manifest){
        [xml]$xml=Get-Content $manifest
        $appId=$aumid.Split('!',2)[1]
        $application=@($xml.Package.Applications.Application | Where-Object { $_.Id -eq $appId })[0]
        if($application){
          $logo=$application.VisualElements.Square44x44Logo
          if(-not $logo){$logo=$application.VisualElements.Square150x150Logo}
          if(-not $logo){$logo=$application.VisualElements.Logo}
          if($logo){
            $candidate=Join-Path $location $logo
            if(Test-Path -LiteralPath $candidate){$icon=$candidate}
            else {
              $assetDir=Split-Path $candidate
              $stem=[IO.Path]::GetFileNameWithoutExtension($candidate)
              $ext=[IO.Path]::GetExtension($candidate)
              $variants=@(Get-ChildItem -LiteralPath $assetDir -File | Where-Object {
                $_.BaseName.StartsWith($stem+'.') -and $_.Extension -eq $ext
              } | Sort-Object @{Expression={ if($_.Name -match 'targetsize-256.*altform-unplated'){0} elseif($_.Name -match 'targetsize-256'){1} elseif($_.Name -match 'targetsize-128'){2} else {3} }},Name)
              if($variants.Count){$icon=$variants[0].FullName}
            }
          }
        }
      }
    }
  }
  $items += [pscustomobject]@{ name=$item.Name; aumid=$aumid; path=$path; icon=$icon }
}
ConvertTo-Json -Compress -Depth 2 -InputObject @($items)"#;
    let output = crate::core::volumes::windows_shell::run(script)?;
    let value: serde_json::Value =
        serde_json::from_str(&output).map_err(|error| error.to_string())?;
    let values = match value {
        serde_json::Value::Array(values) => values,
        serde_json::Value::Null => vec![],
        other => vec![other],
    };
    let mut entries = Vec::new();
    for value in values {
        let Ok(app) = serde_json::from_value::<WindowsApp>(value) else {
            continue;
        };
        if app.name.trim().is_empty() {
            continue;
        }

        let path_is_file = looks_like_path(&app.path);
        let resolved_path = if path_is_file {
            resolve_windows_executable(&app.path)
        } else {
            None
        };
        let id = if looks_like_path(&app.aumid) {
            // A desktop AppsFolder identity may itself be a Known Folder
            // target. Prefer it, but fall back to the item's real path.
            let Some(target) =
                resolve_windows_executable(&app.aumid).or_else(|| resolved_path.clone())
            else {
                continue;
            };
            format!("path:{target}")
        } else if !app.aumid.is_empty() {
            // Packaged apps launch through their AUMID, not a local path.
            // Non-packaged entries with a stale local target are skipped.
            if !app.aumid.contains('!') && path_is_file && resolved_path.is_none() {
                continue;
            }
            format!("aumid:{}", app.aumid)
        } else if let Some(target) = resolved_path {
            format!("path:{target}")
        } else {
            continue;
        };

        let mut item = entry(app.name, format!("{URI}{}", encode(&id)), "app", 0, true);
        // Icon lookup must use the *resolved* launch target, not the original
        // {GUID}\foo.exe token that the Shell image factory cannot parse.
        if !app.icon.is_empty() && Path::new(&app.icon).is_file() {
            item.link_target = Some(app.icon);
        } else if let Some(aumid) = id.strip_prefix("aumid:") {
            item.link_target = Some(format!("shell:AppsFolder\\{aumid}"));
        } else if let Some(target) = id.strip_prefix("path:") {
            item.link_target = Some(target.to_owned());
        }
        entries.push(item);
    }
    entries.sort_by_key(|left| left.name.to_lowercase());
    Ok(entries)
}

#[cfg(target_os = "windows")]
pub(in crate::core::volumes::applications) fn platform_open(encoded: &str) -> Result<(), String> {
    let id = decode(encoded)?;
    let target = if let Some(aumid) = id.strip_prefix("aumid:") {
        format!("shell:AppsFolder\\{aumid}")
    } else if let Some(path) = id.strip_prefix("path:") {
        // Revalidate bookmarked/cached identities at launch, including older
        // paths that still contain a Known Folder GUID.
        resolve_windows_executable(path)
            .ok_or_else(|| crate::i18n::t("无法启动应用").to_string())?
    } else {
        return Err(crate::i18n::t("无法启动应用").into());
    };
    // ShellExecute resolves both `shell:AppsFolder\<AUMID>` and plain binaries.
    // Unlike `cmd /C start` it is detached and never allocates a console window.
    open::that_detached(target).map_err(|error| error.to_string())
}
