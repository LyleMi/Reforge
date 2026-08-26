def normalize_webhook_event(payload: dict) -> dict:
    return {
        "event_id": payload.get("id", ""),
        "event_type": payload.get("type", "unknown").lower(),
    }
