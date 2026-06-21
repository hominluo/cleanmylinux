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
    property var rows: parseRows()

    function parseRows() {
        if (!controllerRef || controllerRef.rows_json.length === 0) {
            return []
        }
        try {
            return JSON.parse(controllerRef.rows_json)
        } catch (e) {
            return []
        }
    }

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
                    enabled: controllerRef && !controllerRef.busy && controllerRef.has_selection
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

        Repeater {
            model: page.rows
            delegate: Kirigami.AbstractCard {
                Layout.fillWidth: true

                contentItem: RowLayout {
                    spacing: Kirigami.Units.smallSpacing

                    Controls.CheckBox {
                        checked: modelData.selected
                        enabled: controllerRef && !controllerRef.busy
                        onToggled: controllerRef.set_selected(index, checked)
                    }

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 2

                        RowLayout {
                            Layout.fillWidth: true
                            Controls.Label {
                                Layout.fillWidth: true
                                text: modelData.label
                                elide: Text.ElideRight
                                font.bold: true
                            }
                            Controls.Label {
                                text: modelData.size_text
                                opacity: 0.72
                            }
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: Kirigami.Units.smallSpacing
                            Controls.Label {
                                text: modelData.category
                                opacity: 0.72
                                elide: Text.ElideRight
                            }
                            Controls.Label {
                                visible: modelData.safety !== "safe"
                                text: modelData.safety === "risky" ? "Risky" : "Review"
                                color: modelData.safety === "risky"
                                       ? Kirigami.Theme.negativeTextColor
                                       : Kirigami.Theme.neutralTextColor
                            }
                            Controls.Label {
                                visible: modelData.delete_mode === "trash"
                                text: "Trash"
                                color: Kirigami.Theme.neutralTextColor
                            }
                            Controls.Label {
                                visible: modelData.privileged
                                text: "Privileged"
                                color: Kirigami.Theme.neutralTextColor
                            }
                            Controls.Label {
                                Layout.fillWidth: true
                                visible: modelData.note.length > 0
                                text: modelData.note
                                opacity: 0.72
                                elide: Text.ElideRight
                            }
                        }
                    }
                }
            }
        }
    }

    Kirigami.PromptDialog {
        id: cleanDialog
        title: "Clean selected items?"
        subtitle: controllerRef ? controllerRef.summary + ". Privileged actions may prompt for your password." : ""
        standardButtons: Controls.Dialog.Ok | Controls.Dialog.Cancel
        onAccepted: controllerRef.clean_safe()
    }
}
