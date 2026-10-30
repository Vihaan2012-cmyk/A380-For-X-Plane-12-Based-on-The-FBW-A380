import bpy

# The same export as the Kit panel; fields left as None keep the panel's value.
FIELDS = {
    'name': 'My Livery',
    'registration': None,
    'airline': None,
    'icao': None,
    'iata': None,
    'export_engine': None,
    'model': None,
    'creator': None,
}

settings = getattr(bpy.context.scene, 'a380_kit', None)
if settings is None:
    raise RuntimeError('Run the "kit_panel.py" text first (Text editor, Run Script)')
for key, value in FIELDS.items():
    if value is not None:
        setattr(settings, key, value)
bpy.ops.a380_kit.export_livery()
