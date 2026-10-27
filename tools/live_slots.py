#!/usr/bin/env python3
"""Per-worker live-test slots shared by the client tools.

Each slot owns two accounts per lane: ``<slot>a`` (the hidden original) and
``<slot>b`` (Bevy / the second client), so two workers never share an account.
The rows are provisioned by ``tools/live-slots/provision.py``; this module only
resolves names and ids for the tools.

Use ``--slot <name>`` or the ``*_SLOT`` environment variable in the tools.
"""
from __future__ import annotations

import os
from dataclasses import dataclass

SLOT_NAMES = [
    "wp1", "wp2", "wp3", "wp4", "wp5", "wp6", "f7", "interaction", "combat",
    "spare1", "spare2", "spare3",
]
ROLES = ("a", "b")
LANES = {
    "move-golden": {"prefix": "Mg", "position": (10512.0, 15585.0, 229.437),
                    "world_id": 0, "zone_id": 182, "level": 1},
    "combat-golden": {"prefix": "Cg", "position": (15528.8, 15335.7, 132.658),
                      "world_id": 0, "zone_id": 179, "level": 30},
}


def slot_index(slot: str) -> int:
    try:
        return SLOT_NAMES.index(slot)
    except ValueError:
        raise SystemExit(
            f"unknown slot {slot!r}; choose one of {', '.join(SLOT_NAMES)}"
        ) from None


@dataclass(frozen=True)
class SlotAccount:
    slot: str
    role: str
    lane: str
    user: str
    character: str
    character_id: int

    @property
    def account_index(self) -> int:
        return slot_index(self.slot) * len(ROLES) + ROLES.index(self.role)


def resolve(slot: str, lane: str, role: str = "a") -> SlotAccount:
    """Names/ids for one role of ``slot`` on ``lane``.

    ``role`` is ``a`` for the hidden original and ``b`` for the second client.
    """
    if slot not in SLOT_NAMES:
        raise SystemExit(
            f"unknown slot {slot!r}; choose one of {', '.join(SLOT_NAMES)}"
        )
    if lane not in LANES:
        raise SystemExit(f"unknown lane {lane!r}; choose one of {', '.join(LANES)}")
    if role not in ROLES:
        raise SystemExit(f"unknown role {role!r}; choose a or b")
    index = slot_index(slot)
    char_id = 1001 + index * len(ROLES) + ROLES.index(role)
    prefix = LANES[lane]["prefix"]
    character = f"{prefix}{slot.capitalize()}{role}"[:16]
    return SlotAccount(slot=slot, role=role, lane=lane, user=f"{slot}{role}",
                       character=character, character_id=char_id)


def lane_for_path(path: str) -> str | None:
    """Lane name from a worktree/live/target path, if it names one."""
    text = str(path).replace("\\", "/").lower()
    for lane in LANES:
        if lane in text:
            return lane
    return None


def env_slot(var: str) -> str | None:
    value = os.environ.get(var)
    return value.strip() or None if value else None
