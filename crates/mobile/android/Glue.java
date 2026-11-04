package app.omochikaeri.hagane;

import android.app.Activity;
import android.app.Fragment;
import android.app.FragmentManager;
import android.content.ClipData;
import android.content.ContentResolver;
import android.content.Context;
import android.content.Intent;
import android.database.Cursor;
import android.graphics.Color;
import android.graphics.Rect;
import android.net.Uri;
import android.os.Build;
import android.os.Environment;
import android.os.VibrationEffect;
import android.os.Vibrator;
import android.provider.DocumentsContract;
import android.provider.OpenableColumns;
import android.util.Log;
import android.view.InputDevice;
import android.view.View;
import android.view.Window;
import android.view.WindowInsets;
import android.view.WindowInsetsController;
import android.view.WindowManager;
import java.io.File;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.util.Arrays;

public class Glue extends Fragment {
    private static final String TAG = "app.omochikaeri.hagane.glue";

    private static final Uri DOWNLOADS_ROOT = DocumentsContract.buildRootUri("com.android.providers.downloads.documents", "downloads");

    private static final int EDGE_DP = 44;

    private static final int RESERVED_DP = 200;

    private static boolean fullscreen;

    private static boolean watchingLayout;

    private static native void picked(int request, String[] uris);

    private static native void shown(boolean visible);

    private static Glue attach(Activity activity) {
        FragmentManager manager = activity.getFragmentManager();
        Glue glue = (Glue) manager.findFragmentByTag(TAG);
        if (glue == null) {
            glue = new Glue();
            manager.beginTransaction().add(glue, TAG).commitNowAllowingStateLoss();
        }
        return glue;
    }

    public static void pick(final Activity activity, final boolean multiple, final int request) {
        activity.runOnUiThread(new Runnable() {
            @Override
            public void run() {
                Intent intent = new Intent(Intent.ACTION_OPEN_DOCUMENT);
                intent.addCategory(Intent.CATEGORY_OPENABLE);
                intent.setType("*/*");
                intent.putExtra(DocumentsContract.EXTRA_INITIAL_URI, DOWNLOADS_ROOT);
                if (multiple) {
                    intent.putExtra(Intent.EXTRA_ALLOW_MULTIPLE, true);
                }
                attach(activity).startActivityForResult(intent, request);
            }
        });
    }

    public static void save(final Activity activity, final String name, final String mime, final int request) {
        activity.runOnUiThread(new Runnable() {
            @Override
            public void run() {
                Intent intent = new Intent(Intent.ACTION_CREATE_DOCUMENT);
                intent.addCategory(Intent.CATEGORY_OPENABLE);
                intent.setType(mime);
                intent.putExtra(DocumentsContract.EXTRA_INITIAL_URI, DOWNLOADS_ROOT);
                intent.putExtra(Intent.EXTRA_TITLE, name);
                attach(activity).startActivityForResult(intent, request);
            }
        });
    }

    public static void bars(final Activity activity, final boolean hidden) {
        activity.runOnUiThread(new Runnable() {
            @Override
            public void run() {
                fullscreen = hidden;
                attach(activity);
                applyBars(activity);
                View decor = activity.getWindow().getDecorView();
                if (!watchingLayout) {
                    watchingLayout = true;
                    decor.addOnLayoutChangeListener(new View.OnLayoutChangeListener() {
                        @Override
                        public void onLayoutChange(View view, int left, int top, int right, int bottom, int oldLeft, int oldTop, int oldRight, int oldBottom) {
                            reserveEdges(view);
                        }
                    });
                }
                reserveEdges(decor);
            }
        });
    }

    private static void reserveEdges(View decor) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.Q || decor.getWidth() == 0) {
            return;
        }
        float density = decor.getResources().getDisplayMetrics().density;
        int wide = Math.round(EDGE_DP * density);
        int tall = Math.round(RESERVED_DP * density);
        int top = Math.max(0, (decor.getHeight() - tall) / 2);
        try {
            decor.setSystemGestureExclusionRects(Arrays.asList(
                new Rect(0, top, wide, top + tall),
                new Rect(decor.getWidth() - wide, top, decor.getWidth(), top + tall)));
        } catch (RuntimeException refused) {
            Log.w(TAG, "the screen edges could not be reserved from the back gesture", refused);
        }
    }

    private static void applyBars(Activity activity) {
        Window window = activity.getWindow();
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
            WindowManager.LayoutParams params = window.getAttributes();
            params.layoutInDisplayCutoutMode = Build.VERSION.SDK_INT >= Build.VERSION_CODES.R
                ? WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_ALWAYS
                : WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES;
            window.setAttributes(params);
        }
        window.setStatusBarColor(Color.TRANSPARENT);
        window.setNavigationBarColor(Color.TRANSPARENT);
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            window.setDecorFitsSystemWindows(false);
            WindowInsetsController controller = window.getInsetsController();
            if (controller != null) {
                controller.setSystemBarsBehavior(WindowInsetsController.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE);
                controller.setSystemBarsAppearance(0, WindowInsetsController.APPEARANCE_LIGHT_STATUS_BARS | WindowInsetsController.APPEARANCE_LIGHT_NAVIGATION_BARS);
                if (fullscreen) {
                    controller.hide(WindowInsets.Type.systemBars());
                } else {
                    controller.show(WindowInsets.Type.systemBars());
                }
            }
        } else {
            int edge = View.SYSTEM_UI_FLAG_LAYOUT_STABLE
                | View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION
                | View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN;
            int hide = View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY
                | View.SYSTEM_UI_FLAG_HIDE_NAVIGATION
                | View.SYSTEM_UI_FLAG_FULLSCREEN;
            window.getDecorView().setSystemUiVisibility(fullscreen ? edge | hide : edge);
        }
    }

    public static int smallestWidthDp(Activity activity) {
        return activity.getResources().getConfiguration().smallestScreenWidthDp;
    }

    public static void vibrate(Activity activity, float strength, int millis) {
        Vibrator vibrator = (Vibrator) activity.getSystemService(Context.VIBRATOR_SERVICE);
        if (vibrator == null || !vibrator.hasVibrator() || millis <= 0) {
            return;
        }
        vibrator.cancel();
        int amplitude = Math.max(1, Math.min((int) (strength * 255.0f), 255));
        vibrator.vibrate(VibrationEffect.createOneShot(millis, amplitude));
    }

    public static void cancelVibration(Activity activity) {
        Vibrator vibrator = (Vibrator) activity.getSystemService(Context.VIBRATOR_SERVICE);
        if (vibrator != null) {
            vibrator.cancel();
        }
    }

    public static boolean hasVibrator(Activity activity) {
        Vibrator vibrator = (Vibrator) activity.getSystemService(Context.VIBRATOR_SERVICE);
        return vibrator != null && vibrator.hasVibrator();
    }

    public static boolean hasMouse() {
        for (int id : InputDevice.getDeviceIds()) {
            InputDevice device = InputDevice.getDevice(id);
            if (device != null && device.supportsSource(InputDevice.SOURCE_MOUSE)) {
                return true;
            }
        }
        return false;
    }

    @Override
    public void onResume() {
        super.onResume();
        applyBars(getActivity());
        shown(true);
    }

    @Override
    public void onPause() {
        super.onPause();
        shown(false);
    }

    @Override
    public void onActivityResult(int request, int result, Intent data) {
        String[] uris = null;
        if (result == Activity.RESULT_OK && data != null) {
            ClipData clip = data.getClipData();
            if (clip != null) {
                uris = new String[clip.getItemCount()];
                for (int i = 0; i < uris.length; i++) {
                    uris[i] = clip.getItemAt(i).getUri().toString();
                }
            } else if (data.getData() != null) {
                uris = new String[] { data.getData().toString() };
            }
        }
        picked(request, uris);
    }

    public static String copy(Activity activity, String uri, String folder) throws IOException {
        ContentResolver resolver = activity.getContentResolver();
        Uri parsed = Uri.parse(uri);
        String name = null;
        try {
            Cursor cursor = resolver.query(parsed, new String[] { OpenableColumns.DISPLAY_NAME }, null, null, null);
            if (cursor != null) {
                try {
                    if (cursor.moveToFirst()) {
                        name = cursor.getString(0);
                    }
                } finally {
                    cursor.close();
                }
            }
        } catch (RuntimeException unnamed) {
            name = null;
        }
        if (name == null || name.isEmpty()) {
            String segment = parsed.getLastPathSegment();
            if (segment != null) {
                int cut = Math.max(segment.lastIndexOf('/'), segment.lastIndexOf(':'));
                name = cut >= 0 ? segment.substring(cut + 1) : segment;
            }
        }
        if (name == null || name.isEmpty()) {
            name = "picked";
        }
        File target = new File(folder, name.replace('/', '_'));
        File parent = target.getParentFile();
        if (parent != null) {
            parent.mkdirs();
        }
        InputStream in = open(resolver, parsed);
        try (OutputStream out = new FileOutputStream(target)) {
            pump(in, out);
        } finally {
            in.close();
        }
        return target.getAbsolutePath();
    }

    private static InputStream open(ContentResolver resolver, Uri parsed) throws IOException {
        String failure;
        try {
            InputStream in = resolver.openInputStream(parsed);
            if (in != null) {
                return in;
            }
            failure = "the provider returned no stream";
        } catch (RuntimeException | IOException streamless) {
            failure = "the provider failed to open it: " + streamless;
        }
        File direct = direct(parsed);
        if (direct != null) {
            try {
                return new FileInputStream(direct);
            } catch (IOException unreadable) {
                failure = failure + "; the path " + direct + " could not be read: " + unreadable;
            }
        }
        throw new IOException(parsed + ": " + failure);
    }

    private static File direct(Uri parsed) {
        String authority = parsed.getAuthority();
        String id;
        try {
            id = DocumentsContract.getDocumentId(parsed);
        } catch (RuntimeException undocumented) {
            return null;
        }
        if (authority == null || id == null) {
            return null;
        }
        if (authority.equals("com.android.externalstorage.documents") && id.startsWith("primary:")) {
            return new File(Environment.getExternalStorageDirectory(), id.substring("primary:".length()));
        }
        if (authority.equals("com.android.providers.downloads.documents") && id.startsWith("raw:")) {
            return new File(id.substring("raw:".length()));
        }
        return null;
    }

    public static void write(Activity activity, String uri, String path) throws IOException {
        ContentResolver resolver = activity.getContentResolver();
        OutputStream out = resolver.openOutputStream(Uri.parse(uri), "wt");
        if (out == null) {
            throw new IOException("the document could not be opened for writing");
        }
        try (InputStream in = new FileInputStream(path)) {
            pump(in, out);
        } finally {
            out.close();
        }
    }

    private static void pump(InputStream in, OutputStream out) throws IOException {
        byte[] buffer = new byte[1 << 16];
        int read;
        while ((read = in.read(buffer)) > 0) {
            out.write(buffer, 0, read);
        }
    }
}
