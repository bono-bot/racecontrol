"""
RaceControl AC Plugin — Shared memory structure definitions.

This plugin runs INSIDE the Assetto Corsa process and writes telemetry
to a custom shared memory buffer ("Local\\rcpmf_telemetry") that rc-agent
reads safely. This avoids the crash-on-exit bug caused by rc-agent reading
AC's own shared memory directly with raw pointers.

Inspired by SRL VMS Connect's architecture but built for RaceControl.
(c) Racing Point eSports 2026
"""

import ctypes
from ctypes import c_int32, c_float, c_wchar

# AC status constants
AC_OFF = 0
AC_REPLAY = 1
AC_LIVE = 2
AC_PAUSE = 3

# AC session type constants
AC_PRACTICE = 0
AC_QUALIFY = 1
AC_RACE = 2
AC_HOTLAP = 3

# Shared memory status protocol — prevents reader/writer races
RC_SM_UNINITIALIZED = 0   # Plugin not loaded or crashed
RC_SM_WRITING = 1         # Plugin is updating — don't read
RC_SM_IDLE = 2            # Safe to read
RC_SM_SHUTDOWN = 3        # Plugin shutting down — stop reading

# Max string sizes (matching AC's wchar arrays)
RC_WCHAR_SHORT = 15
RC_WCHAR_LONG = 33

# Shared memory name — rc-agent opens this instead of acpmf_*
RC_SHM_NAME = "Local\\rcpmf_telemetry"


class RcTelemetryPage(ctypes.Structure):
    """
    Single shared memory page written by the RC AC plugin, read by rc-agent.

    Layout is intentionally flat (no nested structs) for maximum compatibility
    and simple offset-based reading from Rust.
    """
    _pack_ = 4
    _fields_ = [
        # ─── Protocol header ─────────────────────────────────────────
        ('mem_status', c_int32),          # RC_SM_* constant
        ('plugin_version', c_int32),      # Plugin version (increments on update)
        ('plugin_pid', c_int32),          # AC process PID (for liveness check)
        ('update_counter', c_int32),      # Increments every update tick

        # ─── Session info (from AC shared memory + AC API) ───────────
        ('ac_status', c_int32),           # AC_OFF/REPLAY/LIVE/PAUSE
        ('session_type', c_int32),        # AC_PRACTICE/QUALIFY/RACE/HOTLAP
        ('car_on_track', c_int32),        # 1=on track, 2=off track, 0=unknown
        ('is_in_pit', c_int32),           # 1=in pit lane
        ('is_in_pit_lane', c_int32),      # 1=in pit lane approach

        # ─── Static info (set once per session) ──────────────────────
        ('car_model', c_wchar * RC_WCHAR_LONG),
        ('track_name', c_wchar * RC_WCHAR_LONG),
        ('track_config', c_wchar * RC_WCHAR_LONG),
        ('driver_name', c_wchar * RC_WCHAR_LONG),
        ('track_length', c_float),
        ('max_rpm', c_int32),
        ('sector_count', c_int32),

        # ─── Dynamic telemetry (updated every tick ~60Hz) ────────────
        ('speed_kmh', c_float),
        ('rpm', c_int32),
        ('gear', c_int32),               # 0=R, 1=N, 2=1st, 3=2nd...
        ('throttle', c_float),            # 0.0-1.0
        ('brake', c_float),              # 0.0-1.0
        ('steer_angle', c_float),
        ('fuel', c_float),

        # ─── Lap data ────────────────────────────────────────────────
        ('completed_laps', c_int32),
        ('current_lap_time_ms', c_int32),
        ('last_lap_time_ms', c_int32),
        ('best_lap_time_ms', c_int32),
        ('current_sector_index', c_int32),
        ('last_sector_time_ms', c_int32),
        ('lap_invalid', c_int32),         # 1=invalid (off-track cut)

        # ─── Position on track ───────────────────────────────────────
        ('normalized_car_position', c_float),  # 0.0-1.0 spline progress
        ('car_x', c_float),              # World X coordinate
        ('car_y', c_float),              # World Y (height)
        ('car_z', c_float),              # World Z coordinate

        # ─── Session timing ──────────────────────────────────────────
        ('session_time_left', c_float),   # Seconds remaining
        ('number_of_laps', c_int32),      # Total laps in session (0=unlimited)

        # ─── Multiplayer ─────────────────────────────────────────────
        ('server_cars_count', c_int32),   # Total cars in server
        ('position', c_int32),            # Race position (1-based)
    ]
