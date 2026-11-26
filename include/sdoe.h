/*
 * sdoe - design-of-experiments engine, C interface.
 *
 * The library holds all calculations and project data. Each command takes a
 * method name and a JSON object of parameters and returns a JSON object:
 *
 *   {"ok":true,"result":<value>,"changed":<bool>,"revision":<n>}
 *   {"ok":false,"error":"<message>"}
 *
 * "changed" reports whether the project data changed. File access is left to
 * the caller: "project.save" returns the file text and "project.open" takes
 * it; "export.csv" and "export.report" return CSV and HTML text.
 *
 * Common methods (see README.md for the full list and parameters):
 *   project.new, project.open {text}, project.save, project.state,
 *   project.example {name: "reactor"|"blend"}, edit.undo, edit.redo,
 *   sheet.get {sheet: "factors"|"responses"|"runs"},
 *   sheet.set {sheet,row,col,text}, sheet.paste {sheet,row,col,text},
 *   design.methods, design.preview {method,options}, design.apply {mode},
 *   design.next_run {env,mode}, model.set {response,formula,transform},
 *   model.select {response,method}, model.auto {response},
 *   analysis.fit {response}, analysis.equation {response,units,syntax},
 *   analysis.plot {response,kind,...}, optimize.run {env,fixed},
 *   optimize.control_law {factor}, evaluate.run {formula|preset}.
 *
 * Thread safety: calls on one engine are serialised internally.
 * Strings returned by the library must be released with sdoe_string_free.
 */
#ifndef SDOE_H
#define SDOE_H

#ifdef __cplusplus
extern "C" {
#endif

typedef struct SdoeEngine SdoeEngine;

/* Creates an engine with an empty project. */
SdoeEngine *sdoe_engine_new(void);

/* Destroys an engine. NULL is ignored. */
void sdoe_engine_free(SdoeEngine *engine);

/* Runs a command. params_json may be NULL or "" for no parameters.
 * Returns a newly allocated UTF-8 JSON string. */
char *sdoe_call(SdoeEngine *engine, const char *method, const char *params_json);

/* Releases a string returned by sdoe_call or sdoe_version. */
void sdoe_string_free(char *s);

/* Library version, e.g. "0.1.0". Release with sdoe_string_free. */
char *sdoe_version(void);

#ifdef __cplusplus
}
#endif

#endif /* SDOE_H */
