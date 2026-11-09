"""Small, intentionally mixed examples for the manual skill application checks."""


def retry_timeout(fetch, attempts=3):
    for attempt in range(attempts):
        try:
            return fetch()
        except TimeoutError:
            if attempt == attempts - 1:
                raise


def optional_preview(fetch):
    try:
        return fetch()
    except TimeoutError:
        return []


def required_sync(fetch):
    try:
        return fetch()
    except Exception:
        return []


def commit_sync(fetch, commit):
    items = required_sync(fetch)
    commit(items)
    return items
