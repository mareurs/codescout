"""Pricing package.

The renaming re-export below is the fixture for
`docs/issues/2026-08-25-references-dead-ends-at-renaming-re-export.md`:
`apply_duty` is a real binding that no language server emits as a document
symbol, so name-based lookup cannot reach it even though the position resolves.
"""

from library.pricing.duties import duty_multiplier as apply_duty

__all__ = ["apply_duty"]
