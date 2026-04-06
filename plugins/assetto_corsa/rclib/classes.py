"""
RaceControl AC Plugin — Shared memory structure definitions.

VMS-compatible telemetry pipeline: plugin writes to custom shared memory
("Local\\rcpmf_telemetry") that rc-agent reads safely. Supports 64 cars,
live leaderboard, camera control, and on-track detection.

Based on SRL VMS Connect plugin architecture (SPageFile_VMSC_AC).
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
AC_UNKNOWN = -1
AC_PRACTICE = 0
AC_QUALIFY = 1
AC_RACE = 2
AC_HOTLAP = 3
AC_TIME_ATTACK = 4
AC_DRIFT = 5
AC_DRAG = 6

# Shared memory status protocol (VMS-compatible: 6 states)
RC_SM_UNINITIALIZED = 0   # Plugin not loaded or crashed
RC_SM_BLANKED = 1         # Screen is blanked (VMS compat)
RC_SM_WRITING = 2         # Plugin is updating — don't read
RC_SM_READING = 3         # Reader has locked (VMS compat)
RC_SM_IDLE = 4            # Safe to read
RC_SM_INVALID = 5         # Data corrupt / plugin error (VMS compat)
RC_SM_SHUTDOWN = 6        # Plugin shutting down — stop reading

# Max string sizes (matching AC's wchar arrays)
RC_WCHAR_SHORT = 15
RC_WCHAR_LONG = 33

# Max cars (VMS uses 64)
RC_MAX_CARS = 64

# Shared memory name — rc-agent opens this instead of acpmf_*
RC_SHM_NAME = "Local\\rcpmf_telemetry"


class CarData(ctypes.Structure):
    """Per-car telemetry data (VMS CarData equivalent)."""
    _pack_ = 4
    _fields_ = [
        ('car_id', c_int32),
        ('is_connected', c_int32),
        ('is_ai_controlled', c_int32),
        ('is_car_in_pitlane', c_int32),
        ('is_car_in_pit', c_int32),
        ('driver_name', c_wchar * RC_WCHAR_LONG),
        ('leaderboard_position', c_int32),
        ('realtime_leaderboard_position', c_int32),
        ('lap_count', c_int32),
        ('lap_time', c_int32),       # Current lap time (ms)
        ('last_lap', c_int32),       # Last lap time (ms)
        ('best_lap', c_int32),       # Best lap time (ms)
        ('lap_invalid', c_int32),    # 1=cut/invalid
        ('spline_position', c_float),  # 0.0-1.0 normalized
        ('speed_ms', c_float),       # Speed in m/s
        ('gas', c_float),            # 0.0-1.0
        ('brake', c_float),          # 0.0-1.0
        ('clutch', c_float),         # 0.0-1.0
        ('gear', c_int32),
        ('race_finished', c_int32),
    ]


class BoardData(ctypes.Structure):
    """Sorted leaderboard entry (VMS BoardData equivalent)."""
    _pack_ = 4
    _fields_ = [
        ('car_id', c_int32),
        ('position', c_int32),
        ('name', c_wchar * RC_WCHAR_LONG),
        ('lap_count', c_int32),
        ('full_distance', c_float),      # lapCount + splinePosition
        ('spline_position', c_float),
        ('speed_ms', c_float),
        ('time_behind_leader', c_float),
        ('laps_behind_leader', c_int32),
        ('time_behind_ahead', c_float),
        ('laps_behind_ahead', c_int32),
        ('has_crossed_sf', c_int32),     # Start/Finish line crossed
    ]


class RcTelemetryPage(ctypes.Structure):
    """
    Full shared memory page — VMS SPageFile_VMSC_AC equivalent.

    Contains: status protocol, session info, 64-car telemetry array,
    sorted leaderboard, camera control, and player-specific telemetry.
    """
    _pack_ = 4
    _fields_ = [
        # ─── Protocol header (VMS-compatible) ────────────────────────
        ('mem_status', c_int32),          # RC_SM_* constant (6 states)
        ('plugin_version', c_int32),      # Plugin version
        ('plugin_pid', c_int32),          # AC process PID
        ('update_counter', c_int32),      # Increments every update tick
        ('plugin_active', c_int32),       # 1=active, 0=disabled/crashed

        # ─── Session info ────────────────────────────────────────────
        ('ac_status', c_int32),           # AC_OFF/REPLAY/LIVE/PAUSE
        ('session_type', c_int32),        # AC_PRACTICE/QUALIFY/RACE/HOTLAP
        ('car_on_track', c_int32),        # 1=on track, 2=off track, 0=unknown
        ('is_in_pit', c_int32),
        ('is_in_pit_lane', c_int32),
        ('delta_time', c_float),          # Frame delta time

        # ─── Track/server info ───────────────────────────────────────
        ('track_name', c_wchar * RC_WCHAR_LONG),
        ('track_config', c_wchar * RC_WCHAR_LONG),
        ('track_length', c_float),
        ('server_name', c_wchar * RC_WCHAR_LONG),
        ('server_slot_count', c_int32),
        ('server_cars_count', c_int32),

        # ─── Camera control (bidirectional — VMS pattern) ────────────
        ('camera_mode', c_int32),
        ('focused_car', c_int32),
        ('set_camera_car', c_int32),      # rc-agent writes, plugin reads
        ('enable_set_camera_car', c_int32),  # 1=apply set_camera_car

        # ─── Static car info (set once per session) ──────────────────
        ('car_model', c_wchar * RC_WCHAR_LONG),
        ('driver_name', c_wchar * RC_WCHAR_LONG),
        ('max_rpm', c_int32),
        ('sector_count', c_int32),

        # ─── Player telemetry (updated every tick ~60Hz) ─────────────
        ('speed_kmh', c_float),
        ('rpm', c_int32),
        ('gear', c_int32),
        ('throttle', c_float),
        ('brake', c_float),
        ('steer_angle', c_float),
        ('fuel', c_float),

        # ─── Player lap data ─────────────────────────────────────────
        ('completed_laps', c_int32),
        ('current_lap_time_ms', c_int32),
        ('last_lap_time_ms', c_int32),
        ('best_lap_time_ms', c_int32),
        ('current_sector_index', c_int32),
        ('last_sector_time_ms', c_int32),
        ('lap_invalid', c_int32),

        # ─── Position on track ───────────────────────────────────────
        ('normalized_car_position', c_float),
        ('car_x', c_float),
        ('car_y', c_float),
        ('car_z', c_float),

        # ─── Session timing ──────────────────────────────────────────
        ('session_time_left', c_float),
        ('number_of_laps', c_int32),
        ('position', c_int32),

        # ─── Multi-car array (VMS: 64 cars) ──────────────────────────
        ('cars', CarData * RC_MAX_CARS),

        # ─── Sorted leaderboard (VMS: 64 entries) ────────────────────
        ('board', BoardData * RC_MAX_CARS),
    ]
