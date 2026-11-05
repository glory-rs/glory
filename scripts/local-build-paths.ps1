# Query Cargo in the owning workspace without overriding caller configuration.
function Get-NormalCargoTarget {
    param([string]$ManifestPath = (Join-Path $PSScriptRoot '../Cargo.toml'))
    $manifest = [IO.Path]::GetFullPath($ManifestPath)
    Push-Location (Split-Path -Parent $manifest)
    try {
        $metadata = & cargo metadata --manifest-path $manifest --no-deps --format-version 1 | ConvertFrom-Json
        if ($LASTEXITCODE -ne 0) { throw 'cargo metadata failed' }
        return [IO.Path]::GetFullPath($metadata.target_directory).Replace('\', '/')
    } finally { Pop-Location }
}
