param(
    [string]$Target = "all"
)

function Build-Master {
    Write-Host "=== Baue Rust Master ===" -ForegroundColor Cyan
    Push-Location master
    cargo build --release
    if ($LASTEXITCODE -eq 0) {
        Write-Host "Master Build erfolgreich!" -ForegroundColor Green
        Write-Host "Binary: master\target\release\ignite-master.exe" -ForegroundColor Green
    } else {
        Write-Host "Master Build fehlgeschlagen!" -ForegroundColor Red
    }
    Pop-Location
}

function Build-Plugin {
    Write-Host "=== Baue Paper Plugin ===" -ForegroundColor Cyan
    Push-Location plugin
    if (-not (Test-Path "gradlew.bat")) {
        Write-Host "Generiere Gradle Wrapper..." -ForegroundColor Yellow
        gradle wrapper --gradle-version 8.10
    }
    .\gradlew.bat build
    if ($LASTEXITCODE -eq 0) {
        Write-Host "Plugin Build erfolgreich!" -ForegroundColor Green
        Write-Host "JAR: plugin\build\libs\ignite-plugin-1.0.0.jar" -ForegroundColor Green
    } else {
        Write-Host "Plugin Build fehlgeschlagen!" -ForegroundColor Red
    }
    Pop-Location
}

switch ($Target.ToLower()) {
    "master" { Build-Master }
    "plugin" { Build-Plugin }
    "all" {
        Build-Master
        Write-Host ""
        Build-Plugin
    }
    default {
        Write-Host "Unbekanntes Target: $Target"
        Write-Host "Verwendung: .\build.ps1 [-Target master|plugin|all]"
    }
}
