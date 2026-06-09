# Serve one benchmark app with trunk in release mode.
#   ./run.ps1 glory | leptos | dioxus
param(
  [Parameter(Mandatory = $true)]
  [ValidateSet('glory', 'leptos', 'dioxus')]
  [string]$App
)

Set-Location (Join-Path $PSScriptRoot $App)
trunk serve --release --open
