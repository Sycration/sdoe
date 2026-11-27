#include "webfile.h"

#include <QtWidgets/QFileDialog>

void sdoeOpenFileContent(const QString &filter, rust::Fn<void(const QString &, const QByteArray &)> done)
{
    QFileDialog::getOpenFileContent(filter, [done](const QString &name, const QByteArray &content) {
        if (!name.isEmpty())
            done(name, content);
    });
}

void sdoeSaveFileContent(const QString &name, const QByteArray &content)
{
    QFileDialog::saveFileContent(content, name);
}
