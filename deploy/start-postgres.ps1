$ErrorActionPreference = 'Stop'
try {
    if ($env:POSTGRES_SERVICE) {
        if ($env:POSTGRES_SERVICE.IndexOfAny([char[]]'*?[]') -ge 0) { throw 'POSTGRES_SERVICE must be an exact service name.' }
        $service = Get-Service -Name $env:POSTGRES_SERVICE
        if ($service.Name -notlike 'postgresql*') { throw 'POSTGRES_SERVICE must name a PostgreSQL service.' }
    } else {
        $services = @(Get-Service -Name 'postgresql*' -ErrorAction SilentlyContinue)
        if ($services.Count -eq 0) { throw 'No PostgreSQL service found. Install PostgreSQL with its Windows service first.' }
        if ($services.Count -gt 1) { throw 'Multiple PostgreSQL services found. Set POSTGRES_SERVICE in .env to the one you want.' }
        $service = $services[0]
    }
    if ($service.Status -ne 'Running') {
        try { Start-Service -Name $service.Name } catch {
            throw 'Start the PostgreSQL service in Windows Services, or run this launcher as administrator once.'
        }
        $service.WaitForStatus('Running', [TimeSpan]::FromSeconds(30))
    }
    Write-Output ('PostgreSQL service is running: ' + $service.Name)
} catch {
    Write-Host $_.Exception.Message
    exit 1
}
