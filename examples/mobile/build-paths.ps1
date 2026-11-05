function Resolve-MobileBuildRoot([string]$Path) {
    $absolute = [IO.Path]::GetFullPath($Path)
    $ancestor = $absolute
    while ($ancestor) {
        if ((Test-Path -LiteralPath $ancestor) -and ((Get-Item -LiteralPath $ancestor -Force).Attributes -band [IO.FileAttributes]::ReparsePoint)) {
            throw "Mobile build paths cannot pass through a junction/symlink: $ancestor"
        }
        $ancestor = Split-Path -Parent $ancestor
    }
    return $absolute
}
