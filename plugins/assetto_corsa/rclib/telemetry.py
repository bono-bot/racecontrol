"""
RaceControl AC Plugin — Telemetry writer.

Runs inside AC process, writes to shared memory that rc-agent reads safely.
Uses render callback timing (VMS pattern) for reliable on-track detection.

(c) Racing Point eSports 2026
"""

import time
import os
import mmap
import ctypes

import ac
import acsys

from rclib.classes import (
    RcTelemetryPage, RC_SHM_NAME,
    RC_SM_UNINITIALIZED, RC_SM_WRITING, RC_SM_IDLE, RC_SM_SHUTDOWN,
    RC_WCHAR_LONG, AC_LIVE,
)

# If render callback hasn't fired in this many seconds, car is off-track
RENDER_TIMEOUT_SECS = 0.5

PLUGIN_VERSION = 1


class RcTelemetryWriter:
    """Writes telemetry to 'Local\\rcpmf_telemetry' shared memory."""

    def __init__(self):
        self.last_render_time = time.perf_counter()
        self.last_on_track_state = -1
        self._sm_buf = None
        self.sm = None

        try:
            sz = ctypes.sizeof(RcTelemetryPage)
            self._sm_buf = mmap.mmap(0, sz, RC_SHM_NAME.replace("Local\\\\", ""))
            self.sm = RcTelemetryPage.from_buffer(self._sm_buf)

            # Initialize to safe defaults
            self.sm.mem_status = RC_SM_WRITING
            self.sm.plugin_version = PLUGIN_VERSION
            self.sm.plugin_pid = os.getpid()
            self.sm.update_counter = 0
            self.sm.ac_status = 0
            self.sm.car_on_track = 0
            self.sm.mem_status = RC_SM_IDLE

            ac.log("RC Plugin: shared memory initialized ({} bytes)".format(sz))
        except Exception as e:
            ac.log("RC Plugin: FAILED to create shared memory: {}".format(e))
            ac.console("RC Plugin: shared memory error - rc-agent will use fallback")

    def on_render(self, delta_time):
        """Called by AC render callback — tracks last render time for on-track detection."""
        self.last_render_time = time.perf_counter()

    def update(self, delta_time):
        """Called by acUpdate — writes all telemetry to shared memory."""
        if self.sm is None:
            return

        # Set writing flag — rc-agent will wait for IDLE before reading
        self.sm.mem_status = RC_SM_WRITING

        try:
            self._update_session()
            self._update_telemetry()
            self._update_lap_data()
            self._update_position()
            self.sm.update_counter += 1
        except Exception as e:
            ac.log("RC Plugin: update error: {}".format(e))

        # Done writing — safe to read
        self.sm.mem_status = RC_SM_IDLE

    def _update_session(self):
        """Read AC graphics shared memory for session state."""
        try:
            from rclib.classes import AC_LIVE
            # Read AC's own shared memory for graphics status
            _acg = mmap.mmap(0, 800, "acpmf_graphics")
            # status is at offset 4 (i32)
            status_bytes = _acg[4:8]
            ac_status = int.from_bytes(status_bytes, byteorder='little', signed=True)
            # session type at offset 8
            session_bytes = _acg[8:12]
            session_type = int.from_bytes(session_bytes, byteorder='little', signed=True)
            # isInPit at offset 160
            pit_bytes = _acg[160:164]
            is_in_pit = int.from_bytes(pit_bytes, byteorder='little', signed=True)
            _acg.close()

            self.sm.ac_status = ac_status
            self.sm.session_type = session_type
            self.sm.is_in_pit = is_in_pit

            # On-track detection via render callback timing (VMS pattern)
            now = time.perf_counter()
            render_delta = now - self.last_render_time

            if render_delta > RENDER_TIMEOUT_SECS and ac_status == AC_LIVE:
                on_track = 2  # OFF TRACK (render stopped but status is live)
            elif ac_status == AC_LIVE:
                on_track = 1  # ON TRACK
            else:
                on_track = 0  # UNKNOWN (not in live state)

            if on_track != self.last_on_track_state:
                state_name = {0: "Unknown", 1: "On Track", 2: "Off Track"}.get(on_track, "?")
                ac.log("RC Plugin: car state changed -> {} (render_delta={:.2f}s)".format(
                    state_name, render_delta))
                self.last_on_track_state = on_track

            self.sm.car_on_track = on_track

        except Exception as e:
            ac.log("RC Plugin: session update error: {}".format(e))

    def _update_telemetry(self):
        """Read physics data via AC API."""
        try:
            self.sm.speed_kmh = ac.getCarState(0, acsys.CS.SpeedKMH)
            self.sm.rpm = int(ac.getCarState(0, acsys.CS.RPM))
            self.sm.gear = ac.getCarState(0, acsys.CS.Gear)
            self.sm.throttle = ac.getCarState(0, acsys.CS.Gas)
            self.sm.brake = ac.getCarState(0, acsys.CS.Brake)
            self.sm.steer_angle = ac.getCarState(0, acsys.CS.Steer)
            self.sm.fuel = ac.getCarState(0, acsys.CS.Fuel)
        except Exception:
            pass  # Non-fatal — telemetry gaps are acceptable

    def _update_lap_data(self):
        """Read lap timing data via AC API."""
        try:
            self.sm.completed_laps = ac.getCarState(0, acsys.CS.LapCount)
            self.sm.current_lap_time_ms = int(ac.getCarState(0, acsys.CS.LapTime))
            self.sm.last_lap_time_ms = int(ac.getCarState(0, acsys.CS.LastLap))
            self.sm.best_lap_time_ms = int(ac.getCarState(0, acsys.CS.BestLap))
            self.sm.lap_invalid = ac.getCarState(0, acsys.CS.LapInvalidated)
            self.sm.normalized_car_position = ac.getCarState(0, acsys.CS.NormalizedSplinePosition)
        except Exception:
            pass

    def _update_position(self):
        """Read car world position."""
        try:
            coords = ac.getCarState(0, acsys.CS.WorldPosition)
            if coords and len(coords) >= 3:
                self.sm.car_x = coords[0]
                self.sm.car_y = coords[1]
                self.sm.car_z = coords[2]
        except Exception:
            pass

    def set_static_info(self):
        """Set once-per-session static data."""
        if self.sm is None:
            return
        try:
            track = ac.getTrackName(0)
            if track:
                nb = min(len(track), RC_WCHAR_LONG)
                self.sm.track_name = track[:nb]

            self.sm.track_length = ac.getTrackLength(0)
            self.sm.server_cars_count = ac.getCarsCount()

            # Read static shared memory for car model, driver name
            try:
                _acs = mmap.mmap(0, 600, "acpmf_static")
                # carModel at offset 68 (wchar[33])
                car_bytes = _acs[68:68+66]
                car_model = car_bytes.decode('utf-16-le').rstrip('\x00')
                if car_model:
                    nb = min(len(car_model), RC_WCHAR_LONG)
                    self.sm.car_model = car_model[:nb]

                # track at offset 134 (wchar[33])
                track_bytes = _acs[134:134+66]
                track_cfg = track_bytes.decode('utf-16-le').rstrip('\x00')
                if track_cfg:
                    nb = min(len(track_cfg), RC_WCHAR_LONG)
                    self.sm.track_config = track_cfg[:nb]

                # playerName at offset 200 (wchar[33])
                name_bytes = _acs[200:200+66]
                driver = name_bytes.decode('utf-16-le').rstrip('\x00')
                if driver:
                    nb = min(len(driver), RC_WCHAR_LONG)
                    self.sm.driver_name = driver[:nb]

                # maxRpm at offset 398+4*4=414? Let's read from the AC API instead
                _acs.close()
            except Exception as e:
                ac.log("RC Plugin: static shared memory read error: {}".format(e))

        except Exception as e:
            ac.log("RC Plugin: set_static_info error: {}".format(e))

    def shutdown(self):
        """Clean shutdown — set status so rc-agent knows plugin is gone."""
        if self.sm is not None:
            self.sm.mem_status = RC_SM_SHUTDOWN
            self.sm.plugin_pid = 0
            self.sm.car_on_track = 0
            ac.log("RC Plugin: shared memory marked SHUTDOWN")
        if self._sm_buf is not None:
            self._sm_buf.close()
            ac.log("RC Plugin: shared memory closed")
