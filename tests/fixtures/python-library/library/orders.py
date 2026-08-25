"""Order totals — consumes the aliased re-export from `library.pricing`."""

from library.pricing import apply_duty


def landed_cost(subtotal: float) -> float:
    """Total payable once import duty is applied."""
    return subtotal * apply_duty()
