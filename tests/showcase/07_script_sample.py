import re


def parse_rows(lines):
    result = []
    for raw in lines:
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        m = re.match(r"(?P<name>[a-z_]+)=(?P<value>.+)", line)
        if m:
            result.append((m.group("name"), m.group("value")))
    return result


def main():
    rows = [
        "timeout_ms=1200",
        "retry_count=3",
        "feature_flag=true",
    ]

    for key, value in parse_rows(rows):
        print(f"{key}: {value}")


if __name__ == "__main__":
    main()
