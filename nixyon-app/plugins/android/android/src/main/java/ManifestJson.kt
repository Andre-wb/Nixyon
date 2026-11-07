package dev.demo.nixyon.speech

import org.json.JSONObject

// Описания набора в том же формате, что пишут fetch-models.sh и fetch-llm.sh: их читают
// ModelManifest и LlmManifest.
object ManifestJson {
    fun speech(version: String, files: Map<ModelRole, ModelFile>): String {
        val filesJson = JSONObject()
        for ((role, file) in files) filesJson.put(role.key, fileJson(file))
        return JSONObject().put("version", version).put("files", filesJson).toString(2)
    }

    fun llm(manifest: LlmManifest): String =
        JSONObject().put("version", manifest.version).put("model", fileJson(manifest.model)).toString(2)

    private fun fileJson(file: ModelFile): JSONObject =
        JSONObject().put("path", file.path).put("size", file.size).put("sha256", file.sha256)
}
