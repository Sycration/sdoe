// File access through the browser in the WebAssembly build (the desktop
// build opens the system file dialog with the same calls).
#pragma once

#include <QtCore/QByteArray>
#include <QtCore/QString>

#include "rust/cxx.h"

// Asks for a file and passes its name and contents to `done`. A cancelled
// selection calls nothing.
void sdoeOpenFileContent(const QString &filter, rust::Fn<void(const QString &, const QByteArray &)> done);

// Offers `content` as a download named `name`.
void sdoeSaveFileContent(const QString &name, const QByteArray &content);
