# Zyvor Device Agent Plugin SDK (Go)

Emit one JSON object on stdout. Prefer a v2 manifest with `capabilities`.

```go
package main

import (
    "encoding/json"
    "os"
)

func main() {
    _ = json.NewEncoder(os.Stdout).Encode(map[string]any{
        "sensor": "cabinet-temperature",
        "value": 31.5,
        "kind": "temperature",
        "unit": "celsius",
        "quality": "good",
    })
}
```
