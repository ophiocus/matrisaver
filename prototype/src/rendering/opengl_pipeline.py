"""OpenGL post-process pipeline for glow and gamma."""

from __future__ import annotations

import math
from typing import Tuple

import pygame

from config import RuntimeConfig


class OpenGLPipeline:
    def __init__(self, size: Tuple[int, int]) -> None:
        from OpenGL import GL

        self.gl = GL
        self.width, self.height = size
        self.program = self._create_program()
        self._uniforms = self._fetch_uniforms(self.program)
        self._last_config: Tuple[float, float, float, float] | None = None
        self._last_resolution: Tuple[int, int] | None = None
        self._texture_format: int | None = None
        self._fallback_copy = False
        self._can_generate_mipmaps = bool(getattr(self.gl, "glGenerateMipmap", None))
        self.vao = self.gl.glGenVertexArrays(1)
        self.vbo = self.gl.glGenBuffers(1)
        self._setup_quad()
        self.texture = self.gl.glGenTextures(1)
        self._configure_texture(self.width, self.height)
        self.gl.glPixelStorei(self.gl.GL_UNPACK_ALIGNMENT, 1)

    def resize(self, size: Tuple[int, int]) -> None:
        self.width, self.height = size
        self._configure_texture(self.width, self.height)
        self._last_resolution = None

    def present(self, surface: pygame.Surface, config: RuntimeConfig) -> None:
        gl = self.gl
        gl.glViewport(0, 0, self.width, self.height)
        gl.glUseProgram(self.program)
        gl.glBindVertexArray(self.vao)
        gl.glActiveTexture(gl.GL_TEXTURE0)
        gl.glBindTexture(gl.GL_TEXTURE_2D, self.texture)
        glow_mip = self._glow_mip_level(config)
        pixel_format, pixel_data = self._surface_upload(surface)
        gl.glTexSubImage2D(
            gl.GL_TEXTURE_2D,
            0,
            0,
            0,
            self.width,
            self.height,
            pixel_format,
            gl.GL_UNSIGNED_BYTE,
            pixel_data,
        )
        if glow_mip > 0.0 and self._can_generate_mipmaps:
            gl.glGenerateMipmap(gl.GL_TEXTURE_2D)
        self._apply_uniforms(config, glow_mip)
        gl.glClearColor(0.0, 0.0, 0.0, 1.0)
        gl.glClear(gl.GL_COLOR_BUFFER_BIT)
        gl.glDrawArrays(gl.GL_TRIANGLES, 0, 6)
        gl.glBindVertexArray(0)

    def _surface_upload(self, surface: pygame.Surface) -> Tuple[int, object]:
        gl = self.gl
        if self._texture_format is None:
            masks = surface.get_masks()
            if masks == (0x000000FF, 0x0000FF00, 0x00FF0000, 0xFF000000):
                self._texture_format = gl.GL_RGBA
            elif masks == (0x00FF0000, 0x0000FF00, 0x000000FF, 0xFF000000):
                self._texture_format = gl.GL_BGRA
            else:
                self._texture_format = gl.GL_RGBA
                self._fallback_copy = True
        if self._fallback_copy:
            return gl.GL_RGBA, pygame.image.tostring(surface, "RGBA", False)
        return self._texture_format, surface.get_buffer().raw

    def _apply_uniforms(self, config: RuntimeConfig, glow_mip: float) -> None:
        gl = self.gl
        config_key = (
            config.vfx_glow_strength,
            config.vfx_glow_radius,
            config.vfx_glow_threshold,
            config.vfx_gamma,
            glow_mip,
        )
        if config_key != self._last_config:
            gl.glUniform1f(self._uniforms["uGlowStrength"], config.vfx_glow_strength)
            gl.glUniform1f(self._uniforms["uGlowRadius"], config.vfx_glow_radius)
            gl.glUniform1f(self._uniforms["uGlowThreshold"], config.vfx_glow_threshold)
            gl.glUniform1f(self._uniforms["uGamma"], config.vfx_gamma)
            gl.glUniform1f(self._uniforms["uGlowMip"], glow_mip)
            self._last_config = config_key
        if self._last_resolution != (self.width, self.height):
            gl.glUniform2f(self._uniforms["uResolution"], float(self.width), float(self.height))
            self._last_resolution = (self.width, self.height)

    def _configure_texture(self, width: int, height: int) -> None:
        gl = self.gl
        gl.glBindTexture(gl.GL_TEXTURE_2D, self.texture)
        gl.glTexImage2D(
            gl.GL_TEXTURE_2D,
            0,
            gl.GL_RGBA,
            width,
            height,
            0,
            gl.GL_RGBA,
            gl.GL_UNSIGNED_BYTE,
            None,
        )
        gl.glTexParameteri(gl.GL_TEXTURE_2D, gl.GL_TEXTURE_MIN_FILTER, gl.GL_LINEAR_MIPMAP_LINEAR)
        gl.glTexParameteri(gl.GL_TEXTURE_2D, gl.GL_TEXTURE_MAG_FILTER, gl.GL_LINEAR)
        gl.glTexParameteri(gl.GL_TEXTURE_2D, gl.GL_TEXTURE_WRAP_S, gl.GL_CLAMP_TO_EDGE)
        gl.glTexParameteri(gl.GL_TEXTURE_2D, gl.GL_TEXTURE_WRAP_T, gl.GL_CLAMP_TO_EDGE)

    def _setup_quad(self) -> None:
        gl = self.gl
        vertices = (
            -1.0,
            -1.0,
            0.0,
            0.0,
            1.0,
            -1.0,
            1.0,
            0.0,
            1.0,
            1.0,
            1.0,
            1.0,
            -1.0,
            -1.0,
            0.0,
            0.0,
            1.0,
            1.0,
            1.0,
            1.0,
            -1.0,
            1.0,
            0.0,
            1.0,
        )
        vertex_data = (gl.GLfloat * len(vertices))(*vertices)
        gl.glBindVertexArray(self.vao)
        gl.glBindBuffer(gl.GL_ARRAY_BUFFER, self.vbo)
        gl.glBufferData(gl.GL_ARRAY_BUFFER, len(vertices) * 4, vertex_data, gl.GL_STATIC_DRAW)
        gl.glEnableVertexAttribArray(0)
        gl.glVertexAttribPointer(0, 2, gl.GL_FLOAT, gl.GL_FALSE, 16, gl.GLvoidp(0))
        gl.glEnableVertexAttribArray(1)
        gl.glVertexAttribPointer(1, 2, gl.GL_FLOAT, gl.GL_FALSE, 16, gl.GLvoidp(8))
        gl.glBindBuffer(gl.GL_ARRAY_BUFFER, 0)
        gl.glBindVertexArray(0)

    def _create_program(self) -> int:
        gl = self.gl
        vertex_source = """
        #version 330 core
        layout (location = 0) in vec2 aPos;
        layout (location = 1) in vec2 aTex;
        out vec2 vTex;
        void main() {
            vTex = aTex;
            gl_Position = vec4(aPos, 0.0, 1.0);
        }
        """
        fragment_source = """
        #version 330 core
        in vec2 vTex;
        out vec4 FragColor;
        uniform sampler2D uTexture;
        uniform float uGlowStrength;
        uniform float uGlowRadius;
        uniform float uGlowThreshold;
        uniform float uGamma;
        uniform float uGlowMip;
        uniform vec2 uResolution;

        vec3 sampleGlow(vec2 uv, vec2 texel) {
            vec3 color = textureLod(uTexture, uv, uGlowMip).rgb;
            float brightness = dot(color, vec3(0.2126, 0.7152, 0.0722));
            float threshold = clamp(uGlowThreshold, 0.0, 1.0);
            float mask = smoothstep(threshold - 0.15, threshold + 0.05, brightness);
            return color * mask;
        }

        void main() {
            vec2 uv = vec2(vTex.x, 1.0 - vTex.y);
            vec2 texel = 1.0 / uResolution;
            float radius = max(uGlowRadius, 0.001);
            vec2 inner = texel * radius;
            vec2 outer = texel * (radius * 2.5 + 1.0);
            vec3 base = texture(uTexture, uv).rgb;
            vec3 glow = sampleGlow(uv, inner) * 0.24;
            glow += sampleGlow(uv + vec2(inner.x, 0.0), inner) * 0.12;
            glow += sampleGlow(uv - vec2(inner.x, 0.0), inner) * 0.12;
            glow += sampleGlow(uv + vec2(0.0, inner.y), inner) * 0.12;
            glow += sampleGlow(uv - vec2(0.0, inner.y), inner) * 0.12;
            glow += sampleGlow(uv + inner, inner) * 0.08;
            glow += sampleGlow(uv - inner, inner) * 0.08;
            glow += sampleGlow(uv + vec2(inner.x, -inner.y), inner) * 0.08;
            glow += sampleGlow(uv + vec2(-inner.x, inner.y), inner) * 0.08;
            vec3 outerGlow = sampleGlow(uv, outer) * 0.18;
            outerGlow += sampleGlow(uv + vec2(outer.x, 0.0), outer) * 0.10;
            outerGlow += sampleGlow(uv - vec2(outer.x, 0.0), outer) * 0.10;
            outerGlow += sampleGlow(uv + vec2(0.0, outer.y), outer) * 0.10;
            outerGlow += sampleGlow(uv - vec2(0.0, outer.y), outer) * 0.10;
            vec3 color = base + (glow + outerGlow) * uGlowStrength;
            color = pow(max(color, vec3(0.0)), vec3(1.0 / max(uGamma, 0.1)));
            FragColor = vec4(color, 1.0);
        }
        """
        vertex_shader = gl.glCreateShader(gl.GL_VERTEX_SHADER)
        gl.glShaderSource(vertex_shader, vertex_source)
        gl.glCompileShader(vertex_shader)
        fragment_shader = gl.glCreateShader(gl.GL_FRAGMENT_SHADER)
        gl.glShaderSource(fragment_shader, fragment_source)
        gl.glCompileShader(fragment_shader)
        program = gl.glCreateProgram()
        gl.glAttachShader(program, vertex_shader)
        gl.glAttachShader(program, fragment_shader)
        gl.glLinkProgram(program)
        gl.glDeleteShader(vertex_shader)
        gl.glDeleteShader(fragment_shader)
        gl.glUseProgram(program)
        gl.glUniform1i(gl.glGetUniformLocation(program, "uTexture"), 0)
        gl.glEnable(gl.GL_BLEND)
        gl.glBlendFunc(gl.GL_SRC_ALPHA, gl.GL_ONE_MINUS_SRC_ALPHA)
        return program

    def _fetch_uniforms(self, program: int) -> dict[str, int]:
        gl = self.gl
        return {
            "uGlowStrength": gl.glGetUniformLocation(program, "uGlowStrength"),
            "uGlowRadius": gl.glGetUniformLocation(program, "uGlowRadius"),
            "uGlowThreshold": gl.glGetUniformLocation(program, "uGlowThreshold"),
            "uGamma": gl.glGetUniformLocation(program, "uGamma"),
            "uGlowMip": gl.glGetUniformLocation(program, "uGlowMip"),
            "uResolution": gl.glGetUniformLocation(program, "uResolution"),
        }

    def _glow_mip_level(self, config: RuntimeConfig) -> float:
        if (
            config.vfx_glow_strength <= 0.0
            or config.vfx_glow_radius <= 0.0
            or not self._can_generate_mipmaps
        ):
            return 0.0
        return min(4.0, max(0.0, math.log2(max(config.vfx_glow_radius, 1.0))))
