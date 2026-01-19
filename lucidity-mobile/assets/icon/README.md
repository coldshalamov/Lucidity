# App Icon Assets

This directory should contain the app icon images for Lucidity.

## Required Files

1. **app_icon.png** (1024x1024)
   - The main app icon
   - Used for iOS and Android
   - Should be a square PNG with no transparency for iOS

2. **app_icon_foreground.png** (1024x1024)
   - Android adaptive icon foreground layer
   - Should have transparent background
   - Content centered with safe zone margins

## Design Guidelines

- **Background Color**: #0E0F12 (dark terminal background)
- **Primary Color**: #7AA2F7 (blue accent)
- **Secondary Color**: #BB9AF7 (purple accent)
- **Style**: Minimalist terminal/command prompt symbol
- **No text** in the icon

## Generating Icons

Once you have the PNG files in this directory, run:

```bash
cd lucidity-mobile
flutter pub get
flutter pub run flutter_launcher_icons
```

This will generate all the required iOS and Android icon sizes.

## Placeholder

Until custom icons are created, the app uses the default Flutter icon.
To create a quick placeholder:

1. Create a 1024x1024 image with:
   - Dark background (#0E0F12)
   - Terminal cursor symbol (>_) in blue (#7AA2F7)
2. Save as `app_icon.png` in this folder
3. Run the flutter_launcher_icons command above
