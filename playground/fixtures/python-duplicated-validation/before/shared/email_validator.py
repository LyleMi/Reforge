def validate_email(value: str) -> bool:
    normalized = value.strip().lower()
    return "@" in normalized and "." in normalized.split("@")[-1]
