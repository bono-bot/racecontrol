"""
RaceControl AC Plugin — Telemetry writer.

VMS SimLauncher-compatible telemetry pipeline. Writes player + all 64 cars +
live leaderboard to shared memory. Uses render callback timing for on-track
detection (VMS pattern) and spin-wait protocol for reader/writer coordination.

(c) Racing Point eSports 2026
"""

import time
import os
import mmap
import ctypes
import math
import operator

import ac
import acsys

from rclib.classes import (
    RcTelemetryPage, RC_SHM_NAME, CarData, BoardData,
    RC_SM_UNINITIALIZED, RC_SM_BLANKED, RC_SM_WRITING, RC_SM_READING,
    RC_SM_IDLE, RC_SM_INVALID, RC_SM_SHUTDOWN,
    RC_WCHAR_LONG, RC_MAX_CARS, AC_LIVE, AC_RACE,
)

# If render callback hasn't fired in this many seconds, car is off-track (VMS: 0.5s)
RENDER_TIMEOUT_SECS = 0.5

PLUGIN_VERSION = 2  # Bumped for multi-car + leaderboard support


class RcTelemetryWriter:
    """Writes telemetry to shared memory — VMS VMSCONNECT equivalent."""

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
            self.sm.plugin_active = 0  # Set to 1 after full init
            self.sm.mem_status = RC_SM_IDLE

            ac.log("RC Plugin v{}: shared memory initialized ({} bytes)".format(PLUGIN_VERSION, sz))
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

        # VMS SPIN-WAIT PROTOCOL: Wait for reader to finish before writing.
        # Give up after 20ms (10 retries × 2ms) to avoid blocking AC's update loop.
        timeout = 10
        while self.sm.mem_status == RC_SM_READING and timeout > 0:
            timeout -= 1
            time.sleep(0.002)

        self.sm.mem_status = RC_SM_WRITING
        self.sm.delta_time = delta_time

        try:
            self._update_session()
            self._update_telemetry()
            self._update_lap_data()
            self._update_position()
            self._update_all_cars()
            self._handle_camera_control()
            self.sm.update_counter += 1
        except Exception as e:
            ac.log("RC Plugin: update error: {}".format(e))

        # Done writing — safe to read
        self.sm.mem_status = RC_SM_IDLE

    def _update_session(self):
        """Read AC graphics shared memory for session state."""
        try:
            _acg = mmap.mmap(0, 800, "acpmf_graphics")
            status_bytes = _acg[4:8]
            ac_status = int.from_bytes(status_bytes, byteorder='little', signed=True)
            session_bytes = _acg[8:12]
            session_type = int.from_bytes(session_bytes, byteorder='little', signed=True)
            pit_bytes = _acg[160:164]
            is_in_pit = int.from_bytes(pit_bytes, byteorder='little', signed=True)
            _acg.close()

            self.sm.ac_status = ac_status
            self.sm.session_type = session_type
            self.sm.is_in_pit = is_in_pit

            # On-track detection via render callback timing (VMS pattern)
            now = time.perf_counter()
            render_delta = now - self.last_render_time

            # VMS logic: renderDeltaTime > RENDER_TIME_OUT AND status == AC_LIVE
            if render_delta > RENDER_TIMEOUT_SECS and ac_status == AC_LIVE:
                on_track = 2  # OFF TRACK
            elif ac_status == AC_LIVE:
                on_track = 1  # ON TRACK
            else:
                on_track = 0  # UNKNOWN

            if on_track != self.last_on_track_state:
                state_name = {0: "Unknown", 1: "On Track", 2: "Off Track"}.get(on_track, "?")
                ac.log("RC Plugin: car state -> {} (render_delta={:.2f}s)".format(
                    state_name, render_delta))
                self.last_on_track_state = on_track

            self.sm.car_on_track = on_track

        except Exception as e:
            ac.log("RC Plugin: session update error: {}".format(e))

    def _update_telemetry(self):
        """Read player physics data via AC API."""
        try:
            self.sm.speed_kmh = ac.getCarState(0, acsys.CS.SpeedKMH)
            self.sm.rpm = int(ac.getCarState(0, acsys.CS.RPM))
            self.sm.gear = ac.getCarState(0, acsys.CS.Gear)
            self.sm.throttle = ac.getCarState(0, acsys.CS.Gas)
            self.sm.brake = ac.getCarState(0, acsys.CS.Brake)
            self.sm.steer_angle = ac.getCarState(0, acsys.CS.Steer)
            self.sm.fuel = ac.getCarState(0, acsys.CS.Fuel)
        except Exception:
            pass

    def _update_lap_data(self):
        """Read player lap timing data via AC API."""
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

    def _update_all_cars(self):
        """Read telemetry for ALL cars in session (VMS multi-car pattern).

        VMS reads up to 64 cars every update tick. This enables:
        - Live leaderboard with real-time position tracking
        - Spectator circuit viewer (car dots on track map)
        - Gap/delta calculation between any two cars
        """
        try:
            car_count = ac.getCarsCount()
            self.sm.server_cars_count = car_count
            self.sm.server_slot_count = ac.getServerSlotsCount()
            self.sm.camera_mode = ac.getCameraMode()
            self.sm.focused_car = ac.getFocusedCar()

            for i in range(min(car_count, RC_MAX_CARS)):
                car = self.sm.cars[i]
                car.car_id = i

                driver_name = ac.getDriverName(i)
                if driver_name and driver_name != -1:
                    nb = min(len(driver_name), RC_WCHAR_LONG)
                    car.driver_name = driver_name[:nb]

                    car.is_connected = ac.isConnected(i)
                    car.is_ai_controlled = ac.isAIControlled(i)
                    car.is_car_in_pitlane = ac.isCarInPitlane(i)
                    car.is_car_in_pit = ac.isCarInPit(i)

                    car.leaderboard_position = ac.getCarLeaderboardPosition(i)
                    car.realtime_leaderboard_position = ac.getCarRealTimeLeaderboardPosition(i)

                    car.lap_count = ac.getCarState(i, acsys.CS.LapCount)
                    car.lap_time = int(ac.getCarState(i, acsys.CS.LapTime))
                    car.last_lap = int(ac.getCarState(i, acsys.CS.LastLap))
                    car.best_lap = int(ac.getCarState(i, acsys.CS.BestLap))
                    car.lap_invalid = ac.getCarState(i, acsys.CS.LapInvalidated)
                    car.spline_position = ac.getCarState(i, acsys.CS.NormalizedSplinePosition)
                    car.speed_ms = ac.getCarState(i, acsys.CS.SpeedMS)
                    car.gas = ac.getCarState(i, acsys.CS.Gas)
                    car.brake = ac.getCarState(i, acsys.CS.Brake)
                    car.clutch = ac.getCarState(i, acsys.CS.Clutch)
                    car.gear = ac.getCarState(i, acsys.CS.Gear)
                    car.race_finished = ac.getCarState(i, acsys.CS.RaceFinished)
                else:
                    car.car_id = -1
                    car.is_connected = 0

            # Build sorted leaderboard (VMS LiveLeaderboard equivalent)
            self._update_leaderboard(car_count)

        except Exception as e:
            ac.log("RC Plugin: multi-car update error: {}".format(e))

    def _update_leaderboard(self, car_count):
        """Sort cars by distance (lapCount + splinePosition) and compute deltas."""
        try:
            # Collect connected cars with their full distance
            entries = []
            for i in range(min(car_count, RC_MAX_CARS)):
                car = self.sm.cars[i]
                if car.is_connected and car.car_id >= 0:
                    full_dist = car.lap_count + car.spline_position
                    entries.append((i, car, full_dist))

            # Sort by full distance descending (leader first)
            entries.sort(key=lambda x: x[2], reverse=True)

            leader = None
            prev = None
            for pos, (idx, car, dist) in enumerate(entries):
                board = self.sm.board[pos]
                board.car_id = car.car_id
                nb = min(len(car.driver_name), RC_WCHAR_LONG)
                board.name = car.driver_name[:nb]
                board.position = pos + 1
                board.lap_count = car.lap_count
                board.full_distance = dist
                board.spline_position = car.spline_position
                board.speed_ms = car.speed_ms

                if leader is None:
                    leader = (car, dist)
                    board.time_behind_leader = 0.0
                    board.laps_behind_leader = 0
                else:
                    board.laps_behind_leader = leader[0].lap_count - car.lap_count
                    board.time_behind_leader = leader[1] - dist

                if prev is not None:
                    board.laps_behind_ahead = prev[0].lap_count - car.lap_count
                    board.time_behind_ahead = prev[1] - dist
                else:
                    board.time_behind_ahead = 0.0
                    board.laps_behind_ahead = 0

                prev = (car, dist)

        except Exception as e:
            ac.log("RC Plugin: leaderboard error: {}".format(e))

    def _handle_camera_control(self):
        """VMS bidirectional camera control — rc-agent can switch AC camera."""
        try:
            if self.sm.enable_set_camera_car == 1:
                target = self.sm.set_camera_car
                if 0 <= target < RC_MAX_CARS:
                    ac.focusCar(target)
                    self.sm.enable_set_camera_car = 0
                    ac.log("RC Plugin: camera focus set to car {}".format(target))
        except Exception as e:
            ac.log("RC Plugin: camera control error: {}".format(e))

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
            self.sm.server_slot_count = ac.getServerSlotsCount()

            server_name = ac.getServerName()
            if server_name:
                nb = min(len(server_name), RC_WCHAR_LONG)
                self.sm.server_name = server_name[:nb]

            # Read static shared memory for car model, driver name
            try:
                _acs = mmap.mmap(0, 600, "acpmf_static")
                car_bytes = _acs[68:68+66]
                car_model = car_bytes.decode('utf-16-le').rstrip('\x00')
                if car_model:
                    nb = min(len(car_model), RC_WCHAR_LONG)
                    self.sm.car_model = car_model[:nb]

                name_bytes = _acs[200:200+66]
                driver = name_bytes.decode('utf-16-le').rstrip('\x00')
                if driver:
                    nb = min(len(driver), RC_WCHAR_LONG)
                    self.sm.driver_name = driver[:nb]

                _acs.close()
            except Exception as e:
                ac.log("RC Plugin: static shm read error: {}".format(e))

            # Mark plugin as fully active (VMS pluginActive pattern)
            self.sm.plugin_active = 1

        except Exception as e:
            ac.log("RC Plugin: set_static_info error: {}".format(e))

    def shutdown(self):
        """Clean shutdown — VMS pattern: clear all fields, set SHUTDOWN status."""
        if self.sm is not None:
            self.sm.mem_status = RC_SM_WRITING
            self.sm.plugin_version = -1
            self.sm.plugin_pid = 0
            self.sm.car_on_track = 0
            self.sm.plugin_active = 0
            self.sm.mem_status = RC_SM_SHUTDOWN
            ac.log("RC Plugin: shared memory marked SHUTDOWN")
        if self._sm_buf is not None:
            self._sm_buf.close()
            ac.log("RC Plugin: shared memory closed")
