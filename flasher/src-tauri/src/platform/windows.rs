use super::output;
use crate::{
    storage::detector::StorageDevice,
    utils::errors::{Error, Result},
};
use serde_json::Value;
const SCAN: &str = r#"$ErrorActionPreference='Stop'; $items=@(Get-Disk | ForEach-Object { $d=$_; Get-Partition -DiskNumber $d.Number | ForEach-Object { $p=$_; $v=$p | Get-Volume -ErrorAction SilentlyContinue; if($v -and $v.DriveLetter) { [PSCustomObject]@{id=([string]$v.DriveLetter+':');name=$d.FriendlyName;mount=([string]$v.DriveLetter+':\');capacity=[UInt64]$v.Size;available=[UInt64]$v.SizeRemaining;filesystem=[string]$v.FileSystem;removable=([string]$d.BusType -in @('USB','SD','MMC'));system=([bool]$d.IsSystem -or [bool]$d.IsBoot);identity=$(if($d.UniqueId -and $v.UniqueId){[string]$d.UniqueId+':'+[string]$p.Offset+':'+[string]$v.UniqueId}else{''})} } }); ConvertTo-Json -InputObject $items -Compress"#;
pub fn scan() -> Result<Vec<StorageDevice>> {
    let bytes = output(
        "powershell.exe",
        &["-NoProfile", "-NonInteractive", "-Command", SCAN],
    )?;
    let v: Value = serde_json::from_slice(&bytes)?;
    Ok(serde_json::from_value(v)?)
}
fn letter(d: &StorageDevice) -> Result<char> {
    let chars: Vec<_> = d.id.chars().collect();
    if chars.len() != 2 || !chars[0].is_ascii_uppercase() || chars[1] != ':' {
        return Err(Error::Unsafe("Invalid Windows volume identifier".into()));
    }
    Ok(chars[0])
}
pub fn format(d: &StorageDevice) -> Result<()> {
    let drive = letter(d)?;
    if d.capacity > 32 * 1024 * 1024 * 1024 {
        return Err(Error::Unsafe("Windows native FAT32 formatting is limited to 32 GiB here. Use a trusted FAT32 formatter for larger cards, then refresh.".into()));
    }
    // This command receives only an ASCII drive letter, never manifest data or shell text.
    output("powershell.exe",&["-NoProfile","-NonInteractive","-Command",&format!("$ErrorActionPreference='Stop'; Format-Volume -DriveLetter {drive} -FileSystem FAT32 -NewFileSystemLabel NANOBLAST -Confirm:$false -Force")])?;
    Ok(())
}
pub fn eject(d: &StorageDevice) -> Result<()> {
    let drive = letter(d)?;
    output("powershell.exe",&["-NoProfile","-NonInteractive","-Command",&format!("$ErrorActionPreference='Stop'; (New-Object -ComObject Shell.Application).Namespace(17).ParseName('{drive}:').InvokeVerb('Eject')")])?;
    Ok(())
}
