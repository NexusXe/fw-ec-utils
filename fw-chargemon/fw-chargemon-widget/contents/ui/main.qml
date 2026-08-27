// SPDX-License-Identifier: AGPL-3.0-or-later
// Framework Charging Monitor plasmoid – main QML entry point.
//
// Requires:
//   - fw-chargemon D-Bus service running on the system bus
//   - fw-chargemon-query binary installed and available on PATH

import QtQuick
import QtQuick.Layouts
import org.kde.plasma.plasmoid
import org.kde.plasma.components as PlasmaComponents
import org.kde.plasma.plasma5support as P5Support
import org.kde.kirigami as Kirigami

PlasmoidItem {
    id: root

    // ── D-Bus data state ────────────────────────────────────────────────────
    property bool serviceOk:    false
    property int  activePort:   -1
    property int  voltageMaxMv: 0   // millivolts
    property int  currentMaxMa: 0   // milliamps
    property int  maxPowerUw:   0   // microwatts
    property int  battState:    0   // EC_MMAP_BATT_FLAG bitmask

    // Bit definitions from battery.rs / EC headers
    readonly property bool acPresent:    (battState & 0x01) !== 0
    readonly property bool battPresent:  (battState & 0x02) !== 0
    readonly property bool isDischarging:(battState & 0x04) !== 0
    readonly property bool isCharging:   (battState & 0x08) !== 0

    readonly property string chargeStatus: {
        if (!serviceOk)    return "Service unavailable"
        if (isCharging)    return "Charging"
        if (isDischarging) return "Discharging"
        if (acPresent)     return "Fully charged"
        return "On battery"
    }

    readonly property string statusIcon: {
        if (isCharging)    return "battery-charging"
        if (isDischarging) return "battery-discharging"
        if (acPresent)     return "battery-full-charged"
        return "battery-missing"
    }

    Plasmoid.icon: statusIcon

    // ── Data fetching ────────────────────────────────────────────────────────
    P5Support.DataSource {
        id: executable
        engine: "executable"
        connectedSources: []

        onNewData: (source, data) => {
            connectedSources = connectedSources.filter(s => s !== source)
            root.parseOutput(data["stdout"] ?? "")
        }
    }

    function refresh() {
        executable.connectedSources = [...executable.connectedSources, "/usr/local/bin/fw-chargemon-query"]
    }

    function parseOutput(output) {
        let gotOk = false
        for (const line of output.trim().split('\n')) {
            const eq  = line.indexOf('=')
            if (eq < 0) continue
            const key = line.substring(0, eq)
            const val = parseInt(line.substring(eq + 1), 10)
            switch (key) {
                case 'ok':          gotOk = true;              break
                case 'port':        root.activePort   = val;   break
                case 'voltage_max': root.voltageMaxMv = val;   break
                case 'current_max': root.currentMaxMa = val;   break
                case 'max_power':   root.maxPowerUw   = val;   break
                case 'batt_state':  root.battState    = val;   break
            }
        }
        root.serviceOk = gotOk
    }

    Timer {
        interval:         5000
        running:          true
        repeat:           true
        triggeredOnStart: true
        onTriggered:      root.refresh()
    }

    // ── Compact (panel) representation ──────────────────────────────────────
    compactRepresentation: Item {
        // Stretch to content width; panel height is set by the panel itself
        implicitWidth: panelRow.implicitWidth + Kirigami.Units.smallSpacing * 2

        MouseArea {
            anchors.fill: parent
            onClicked: root.expanded = !root.expanded
        }

        RowLayout {
            id: panelRow
            anchors.centerIn: parent
            spacing: Kirigami.Units.smallSpacing

            Kirigami.Icon {
                source: root.statusIcon
                isMask: true
                color: compactLabel.color
                implicitWidth:  Kirigami.Units.iconSizes.small
                implicitHeight: Kirigami.Units.iconSizes.small
            }

            PlasmaComponents.Label {
                id: compactLabel
                text: {
                    if (!root.serviceOk) return "No service"
                    if (root.activePort < 0) return root.chargeStatus
                    const v = (root.voltageMaxMv / 1000).toFixed(0)
                    const a = (root.currentMaxMa / 1000).toFixed(2)
                    return `Port ${root.activePort}  ${v} V / ${a} A`
                }
                font.pixelSize: Kirigami.Units.gridUnit * 0.8
            }
        }
    }

    // ── Full (popup) representation ──────────────────────────────────────────
    fullRepresentation: ColumnLayout {
        spacing: Kirigami.Units.largeSpacing
        implicitWidth: Kirigami.Units.gridUnit * 18

        // ── Charging status header ──────────────────────────────────────────
        RowLayout {
            spacing: Kirigami.Units.largeSpacing
            Layout.fillWidth: true

            Kirigami.Icon {
                source: root.statusIcon
                implicitWidth:  Kirigami.Units.iconSizes.huge
                implicitHeight: Kirigami.Units.iconSizes.huge
            }

            ColumnLayout {
                spacing: 2
                Layout.fillWidth: true

                PlasmaComponents.Label {
                    text: root.chargeStatus
                    font.bold: true
                    font.pixelSize: Kirigami.Units.gridUnit * 1.1
                }

                PlasmaComponents.Label {
                    text: root.activePort >= 0
                        ? `USB-C Port ${root.activePort}`
                        : (root.serviceOk ? "No charger connected" : "fw-chargemon not running")
                    color: Kirigami.Theme.disabledTextColor
                }
            }
        }

        // ── Separator ───────────────────────────────────────────────────────
        Kirigami.Separator { Layout.fillWidth: true }

        // ── Port stats grid (visible only when a port is active) ────────────
        GridLayout {
            visible:      root.serviceOk && root.activePort >= 0
            columns:      2
            rowSpacing:   Kirigami.Units.smallSpacing
            columnSpacing: Kirigami.Units.largeSpacing
            Layout.fillWidth: true

            // Max voltage
            PlasmaComponents.Label {
                text:  "Max Voltage"
                color: Kirigami.Theme.disabledTextColor
            }
            PlasmaComponents.Label {
                text:  `${(root.voltageMaxMv / 1000).toFixed(1)} V`
                horizontalAlignment: Text.AlignRight
                Layout.fillWidth: true
                font.bold: true
            }

            // Max current
            PlasmaComponents.Label {
                text:  "Max Current"
                color: Kirigami.Theme.disabledTextColor
            }
            PlasmaComponents.Label {
                text:  `${(root.currentMaxMa / 1000).toFixed(2)} A`
                horizontalAlignment: Text.AlignRight
                Layout.fillWidth: true
                font.bold: true
            }

            // Max power (only if non-zero)
            PlasmaComponents.Label {
                visible: root.maxPowerUw > 0
                text:  "Max Power"
                color: Kirigami.Theme.disabledTextColor
            }
            PlasmaComponents.Label {
                visible: root.maxPowerUw > 0
                text:  `${(root.maxPowerUw / 1_000_000).toFixed(0)} W`
                horizontalAlignment: Text.AlignRight
                Layout.fillWidth: true
                font.bold: true
            }
        }

        // ── Footer: refresh button ──────────────────────────────────────────
        RowLayout {
            Layout.fillWidth: true

            PlasmaComponents.Label {
                text:  "Refreshes every 5 s"
                color: Kirigami.Theme.disabledTextColor
                font.pixelSize: Kirigami.Units.gridUnit * 0.7
                Layout.fillWidth: true
            }

            PlasmaComponents.ToolButton {
                icon.name: "view-refresh"
                onClicked: root.refresh()

                PlasmaComponents.ToolTip {
                    text: "Refresh now"
                }
            }
        }
    }
}
