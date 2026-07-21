#!/usr/bin/env python3

import json
import re
from pathlib import Path

import yaml


ROOT = Path(__file__).resolve().parent


def load_yaml(path):
    with path.open(encoding="utf-8") as handle:
        return yaml.safe_load(handle)


def anchor(value):
    value = re.sub(r"[^a-z0-9\s-]", "", value.lower())
    return re.sub(r"[\s-]+", "-", value).strip("-")


def is_uncertain(name, value, uncertain):
    if name in uncertain or value is None or value == "":
        return True
    return "[uncertain]" in json.dumps(value, ensure_ascii=False).lower()


def format_value(value, indent=0):
    if isinstance(value, list):
        return format_list(value, indent)
    if isinstance(value, dict):
        return format_dict(value, indent)
    if isinstance(value, bool):
        return "yes" if value else "no"
    return str(value).strip()


def format_list(values, indent):
    prefix = "  " * indent
    lines = []
    for value in values:
        rendered = format_dict(value, indent + 1, " | ") if isinstance(value, dict) else format_value(value, indent + 1)
        lines.append(f"{prefix}- {rendered}")
    return "\n".join(lines)


def format_dict(value, indent, separator="; "):
    return separator.join(
        f"{key}: {format_value(part, indent + 1)}" for key, part in value.items()
    )


def load_records(result_dir):
    records = []
    for path in sorted(result_dir.glob("*.json")):
        with path.open(encoding="utf-8") as handle:
            records.append(json.load(handle))
    return records


def render_toc(records):
    lines = ["## Table of contents", ""]
    for index, record in enumerate(records, 1):
        name = record.get("item_name", f"Item {index}")
        uncertain = set(record.get("uncertain", []))
        confidence = record.get("confidence_tier")
        suffix = "" if is_uncertain("confidence_tier", confidence, uncertain) else f" - Confidence Tier: {format_value(confidence)}"
        lines.append(f"{index}. [{name}](#{anchor(name)}){suffix}")
    return lines


def field_names(fields):
    return {
        field["name"]
        for category in fields
        for field in category.get("fields", [])
    }


def render_category(record, category, uncertain):
    rendered = []
    for field in category.get("fields", []):
        field_name = field["name"]
        value = record.get(field_name)
        if is_uncertain(field_name, value, uncertain):
            continue
        formatted = format_value(value)
        if formatted:
            rendered.extend([f"#### {field_name.replace('_', ' ').title()}", "", formatted, ""])
    if not rendered:
        return []
    return [f"### {category['category']}", "", *rendered]


def render_extras(record, defined):
    extras = {
        key: value
        for key, value in record.items()
        if key not in defined and key not in {"uncertain", "_source_file"}
    }
    if not extras:
        return []
    lines = ["### Other Info", ""]
    for key, value in extras.items():
        lines.extend([f"#### {key.replace('_', ' ').title()}", "", format_value(value), ""])
    return lines


def render_record(record, fields, defined):
    name = record.get("item_name", "Unnamed item")
    uncertain = set(record.get("uncertain", []))
    lines = ["", f"## {name}", ""]
    for category in fields:
        lines.extend(render_category(record, category, uncertain))
    lines.extend(render_extras(record, defined))
    return lines


def main():
    outline = load_yaml(ROOT / "outline.yaml")
    fields = load_yaml(ROOT / "fields.yaml")["field_categories"]
    result_dir = ROOT / outline.get("execution", {}).get("output_dir", "./results")
    records = load_records(result_dir)
    lines = [
        f"# {outline['topic']}",
        "",
        "> **Status:** Research evidence, not a committed detector specification.",
        "> See the [temporary execution plan](execution-plan.md) for the gated implementation proposal.",
        "",
        *render_toc(records),
    ]

    defined = field_names(fields)
    for record in records:
        lines.extend(render_record(record, fields, defined))

    (ROOT / "report.md").write_text("\n".join(lines).rstrip() + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
