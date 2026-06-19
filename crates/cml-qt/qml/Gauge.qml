// A circular reclaimable-space gauge, drawn with a QML Canvas.
import QtQuick
import org.kde.kirigami as Kirigami

Item {
    id: gauge
    property real fraction: 0.0
    property string label: "—"

    onFractionChanged: canvas.requestPaint()

    Canvas {
        id: canvas
        anchors.fill: parent
        onPaint: {
            var ctx = getContext("2d");
            ctx.reset();
            var w = width, h = height;
            var cx = w / 2, cy = h / 2;
            var radius = Math.min(w, h) / 2 - 14;
            var lineW = 16;

            // Track ring.
            ctx.lineWidth = lineW;
            ctx.strokeStyle = Qt.rgba(0.5, 0.5, 0.5, 0.25);
            ctx.beginPath();
            ctx.arc(cx, cy, radius, 0, 2 * Math.PI);
            ctx.stroke();

            // Progress arc from top.
            var start = -Math.PI / 2;
            var end = start + Math.max(0, Math.min(1, gauge.fraction)) * 2 * Math.PI;
            ctx.lineWidth = lineW;
            ctx.lineCap = "round";
            ctx.strokeStyle = Kirigami.Theme.highlightColor;
            ctx.beginPath();
            ctx.arc(cx, cy, radius, start, end);
            ctx.stroke();
        }
    }

    Column {
        anchors.centerIn: parent
        spacing: 2
        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            text: gauge.label
            font.pointSize: 16
            font.bold: true
            color: Kirigami.Theme.textColor
        }
        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            text: "selected"
            font.pointSize: 9
            opacity: 0.6
            color: Kirigami.Theme.textColor
        }
    }

    Connections {
        target: Kirigami.Theme
        function onHighlightColorChanged() { canvas.requestPaint(); }
    }
}
