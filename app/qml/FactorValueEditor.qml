import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Sdoe

// Value editor for one factor: level list for categorical factors, text otherwise.
Item {
    id: root
    property var factor: null
    property string value: ""
    signal edited(string value)
    implicitHeight: loader.implicitHeight
    implicitWidth: 140

    Loader {
        id: loader
        anchors.left: parent.left
        anchors.right: parent.right
        sourceComponent: root.factor && root.factor.categorical ? levels : field
    }
    Component {
        id: levels
        ComboBox {
            model: root.factor.levels
            currentIndex: Math.max(0, root.factor.levels.indexOf(root.value))
            onActivated: { root.value = currentText; root.edited(currentText) }
        }
    }
    Component {
        id: field
        TextField {
            text: root.value
            horizontalAlignment: Text.AlignRight
            selectByMouse: true
            onEditingFinished: if (text !== root.value) { root.value = text; root.edited(text) }
        }
    }
}
