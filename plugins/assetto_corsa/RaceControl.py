"""
RaceControl AC Plugin — Entry point.

AC Python plugin that writes telemetry to a custom shared memory buffer
("rcpmf_telemetry") for safe reading by rc-agent. This replaces direct
raw pointer access to AC's own shared memory which caused agent crashes.

Install: Copy this folder to <AC>/apps/python/RaceControl/

(c) Racing Point eSports 2026
"""

import sys
import os
import ac

# Add our lib path
sys.path.insert(0, os.path.dirname(__file__))

from rclib.telemetry import RcTelemetryWriter

PLUGIN_NAME = "RaceControl"
VERSION = "1.0.0"

writer = None
app_window = 0


def acMain(*args):
    global writer, app_window

    ac.log("{} v{}: starting...".format(PLUGIN_NAME, VERSION))

    try:
        writer = RcTelemetryWriter()
        writer.set_static_info()
        ac.log("{}: telemetry writer initialized".format(PLUGIN_NAME))
    except Exception as e:
        ac.log("{}: FAILED to initialize: {}".format(PLUGIN_NAME, e))
        ac.console("{}: initialization failed - {}".format(PLUGIN_NAME, e))

    # Create a tiny invisible app window (required by AC plugin API)
    app_window = ac.newApp(PLUGIN_NAME)
    ac.setSize(app_window, 1, 1)
    ac.setIconPosition(app_window, 0, -9000)
    ac.setTitle(app_window, "")
    ac.setBackgroundOpacity(app_window, 0.0)
    ac.drawBorder(app_window, 0)
    ac.setVisible(app_window, 0)

    # Register render callback (critical for on-track detection)
    ac.addRenderCallback(app_window, onRenderCallback)

    ac.log("{} v{}: running".format(PLUGIN_NAME, VERSION))
    return PLUGIN_NAME


def acUpdate(delta_time):
    global writer
    if writer is not None:
        writer.update(delta_time)


def onRenderCallback(delta_time):
    global writer
    if writer is not None:
        writer.on_render(delta_time)


def acShutdown():
    global writer
    ac.log("{}: shutting down...".format(PLUGIN_NAME))
    if writer is not None:
        writer.shutdown()
    ac.log("{}: shutdown complete".format(PLUGIN_NAME))
