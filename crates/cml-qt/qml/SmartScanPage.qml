// Smart Scan dashboard: gauge + actions + results list.
import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as Controls
import org.kde.kirigami as Kirigami

Kirigami.ScrollablePage {
    id: page
    title: "Smart Scan"

    // The shared Controller, injected from Main.qml.
    property var controllerRef

    ColumnLayout {
        spacing: Kirigami.Units.largeSpacing
        width: page.width - Kirigami.Units.largeSpacing * 2

        RowLayout {
            Layout.alignment: Qt.AlignHCenter
            spacing: Kirigami.Units.gridUnit * 2

            Gauge {
                Layout.preferredWidth: 220
                Layout.preferredHeight: 220
                fraction: controllerRef ? controllerRef.gauge_value : 0
                label: controllerRef ? controllerRef.summary : "—"
            }

            ColumnLayout {
                spacing: Kirigami.Units.smallSpacing

                Controls.Button {
                    text: controllerRef && controllerRef.busy ? "Scanning…" : "Scan"
                    icon.name: "edit-find"
                    enabled: !(controllerRef && controllerRef.busy)
                    onClicked: controllerRef.scan(0)
                }
                Controls.Button {
                    text: "Clean selected"
                    icon.name: "edit-clear-all"
                    enabled: !(controllerRef && controllerRef.busy)
                    onClicked: cleanDialog.open()
                }
            }
        }

        Controls.Label {
            Layout.fillWidth: true
            horizontalAlignment: Text.AlignHCenter
            text: controllerRef ? controllerRef.status : ""
            opacity: 0.7
        }

        // Results — split the controller's detail string into list rows.
        Repeater {
            model: controllerRef && controllerRef.detail.length > 0
                   ? controllerRef.detail.split("\n") : []
            delegate: Kirigami.AbstractCard {
                Layout.fillWidth: true
                contentItem: Controls.Label {
                    text: modelData
                    elide: Text.ElideRight
                }
            }
        }
    }

    Kirigami.PromptDialog {
        id: cleanDialog
        title: "Clean selected items?"
        subtitle: "Pre-selected safe items will be removed. Files in 'Large & Old' are sent to Trash. Privileged actions will prompt for your password."
        standardButtons: Controls.Dialog.Ok | Controls.Dialog.Cancel
        onAccepted: controllerRef.clean_safe()
    }
}
