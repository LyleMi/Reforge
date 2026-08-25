def process_order(raw_order: dict) -> dict:
    """Deliberately combines several responsibilities for the playground."""
    customer = str(raw_order.get("customer", "")).strip()
    raw_items = raw_order.get("items", [])
    items = []
    for raw_item in raw_items:
        sku = str(raw_item.get("sku", "")).upper()
        quantity = int(raw_item.get("quantity", 0))
        unit_price = float(raw_item.get("unit_price", 0))
        if not sku:
            raise ValueError("an item needs a SKU")
        if quantity <= 0:
            raise ValueError("quantity must be positive")
        items.append({"sku": sku, "quantity": quantity, "unit_price": unit_price})
    if not customer:
        raise ValueError("an order needs a customer")
    subtotal = sum(item["quantity"] * item["unit_price"] for item in items)
    discount = subtotal * 0.1 if subtotal >= 100 else 0
    tax = (subtotal - discount) * 0.08
    total = round(subtotal - discount + tax, 2)
    record = {"customer": customer, "items": items, "total": total}
    record["storage_key"] = f"orders/{customer.lower().replace(' ', '-')}"
    record["persisted"] = True
    return record
