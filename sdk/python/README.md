# Zyvor Device Agent Plugin SDK (Python)

Native plugins remain ordinary executables. Use a v2 manifest with an explicit
capability list. Hardware capabilities must use `runtime = "native"`.

```python
import json, sys

print(json.dumps({
    "sensor": "cabinet-temperature",
    "value": 31.5,
    "kind": "temperature",
    "unit": "celsius",
    "quality": "good",
    "labels": {"bus": "/dev/i2c-1"}
}))
```
