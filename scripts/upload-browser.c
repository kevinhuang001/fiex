/* Minimal WebKitGTK browser used only by the native recording.
 *
 * It is the drop target for "drag a file out of Fiex into a browser". It loads
 * the upload page served by scripts/upload-target.py and, when a native drag is
 * dropped on it, hands the received URI list to that server through the file
 * named by $FIEX_UPLOAD_DROP. The server then compares the bytes it received
 * with the file Fiex exported, so the recording still proves that the drag
 * carried the real file contents.
 *
 * The drop is observed here rather than in the page because WebKitGTK does not
 * expose an external text/uri-list to the DOM: the page receives a drop event
 * with an empty DataTransfer (no files, empty getData), and WebKit opens the
 * dropped file instead, so a page-side upload can never complete. Chromium is not used instead: on a headless X server it asks
 * for the URI list, previews the file in its status bar and then refuses the
 * drop, and a stock Thunar drag into it is refused in exactly the same way.
 *
 * usage: upload-browser <url> [x y width height]
 */
#include <gtk/gtk.h>
#include <stdlib.h>
#include <string.h>
#include <webkit2/webkit2.h>

/* Write the dropped URIs where upload-target.py is watching for them. */
static void on_drop(GtkWidget *widget, GdkDragContext *context, gint x, gint y,
                    GtkSelectionData *data, guint info, guint time,
                    gpointer user_data) {
  (void)widget, (void)context, (void)x, (void)y, (void)info, (void)time,
      (void)user_data;
  const char *sink = getenv("FIEX_UPLOAD_DROP");
  gchar **uris = gtk_selection_data_get_uris(data);
  if (!sink || !uris) {
    if (uris) g_strfreev(uris);
    return;
  }
  GString *lines = g_string_new("");
  for (int i = 0; uris[i]; i++) {
    g_string_append(lines, uris[i]);
    g_string_append_c(lines, '\n');
  }
  if (lines->len) g_file_set_contents(sink, lines->str, -1, NULL);
  g_string_free(lines, TRUE);
  g_strfreev(uris);
}

/* A dropped file is opened by WebKit, so the drop shows up as a navigation to
 * the file's own URL. Record that URL for upload-target.py. */
static void on_load_changed(WebKitWebView *view, WebKitLoadEvent event,
                            gpointer user_data) {
  (void)user_data;
  if (event != WEBKIT_LOAD_STARTED) return;
  const char *sink = getenv("FIEX_UPLOAD_DROP");
  const gchar *uri = webkit_web_view_get_uri(view);
  if (!sink || !uri || !g_str_has_prefix(uri, "file://")) return;
  g_file_set_contents(sink, uri, -1, NULL);
}

int main(int argc, char **argv) {
  gtk_init(&argc, &argv);
  if (argc < 2) {
    g_printerr("usage: upload-browser <url> [x y width height]\n");
    return 2;
  }
  GtkWidget *window = gtk_window_new(GTK_WINDOW_TOPLEVEL);
  gtk_window_set_title(GTK_WINDOW(window), "Browser Upload");
  gtk_window_set_default_size(GTK_WINDOW(window), 900, 820);
  if (argc >= 6)
    gtk_window_move(GTK_WINDOW(window), atoi(argv[2]), atoi(argv[3])),
        gtk_window_resize(GTK_WINDOW(window), atoi(argv[4]), atoi(argv[5]));
  else
    gtk_window_move(GTK_WINDOW(window), 1200, 0);
  GtkWidget *view = webkit_web_view_new();
  gtk_container_add(GTK_CONTAINER(window), view);
  gtk_drag_dest_set(view, GTK_DEST_DEFAULT_ALL, NULL, 0, GDK_ACTION_COPY);
  gtk_drag_dest_add_uri_targets(view);
  g_signal_connect(view, "drag-data-received", G_CALLBACK(on_drop), NULL);
  g_signal_connect(view, "load-changed", G_CALLBACK(on_load_changed), NULL);
  webkit_web_view_load_uri(WEBKIT_WEB_VIEW(view), argv[1]);
  g_signal_connect(window, "delete-event", G_CALLBACK(gtk_main_quit), NULL);
  gtk_widget_show_all(window);
  gtk_main();
  return 0;
}
