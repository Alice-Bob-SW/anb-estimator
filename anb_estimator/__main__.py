import sys


def main() -> int:
    """Entry point for the `anb-estimator` console script and `python -m anb_estimator`."""
    from anb_estimator._native import _cli_main  # ty: ignore[unresolved-import]

    try:
        _cli_main(["anb-estimator", *sys.argv[1:]])
    except RuntimeError as exc:
        print(f"Error: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
