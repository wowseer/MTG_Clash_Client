# Rendert einen Knopf aus Assets/Buttons/*.blend mit der Kamera der Datei als zwei transparente
# Bilder (doppelte Auflösung): <name>_face.png (nur Knopf) und <name>_glow.png (nur Leuchten).
# Aufruf über tools/render_buttons.sh.
import bpy, sys
argv = sys.argv[sys.argv.index("--") + 1:]
out, name = argv[0], argv[1]
s = bpy.context.scene
s.render.resolution_percentage = 200
s.render.film_transparent = True
s.render.image_settings.file_format = 'PNG'
s.render.image_settings.color_mode = 'RGBA'
objs = {o.name: o for o in bpy.data.objects}
def render(visible, suffix):
    for n, o in objs.items():
        if o.type == 'MESH':
            o.hide_render = n not in visible
    s.render.filepath = f"{out}/{name}{suffix}.png"
    bpy.ops.render.render(write_still=True)
    print("WROTE", s.render.filepath)
render({"Button"}, "_face")
render({"Button_Glow"}, "_glow")
