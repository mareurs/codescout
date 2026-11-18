def parser(log: str) -> dict[str, str]:
    import re
    results: dict[str, str] = {}
    pattern = re.compile(r"^test\s+(.+?)\s+\.\.\.\s+(ok|FAILED|ignored)(?:\b.*)?$", re.MULTILINE)
    status_map = {"ok": "pass", "FAILED": "fail", "ignored": "skip"}
    for match in pattern.finditer(log):
        results[match.group(1)] = status_map[match.group(2)]
    return results