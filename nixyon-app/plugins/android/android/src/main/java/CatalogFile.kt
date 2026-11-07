package dev.demo.nixyon.speech

// Файл из каталога: куда он ложится и какая это роль речевого набора (у языковой модели
// роли нет).
class CatalogFile(val target: ModelTarget, val role: ModelRole?, val file: ModelFile)
