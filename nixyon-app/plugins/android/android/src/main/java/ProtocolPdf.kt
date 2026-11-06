package dev.demo.nixyon.speech

import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.Typeface
import android.graphics.pdf.PdfDocument
import android.text.Layout
import android.text.SpannableStringBuilder
import android.text.Spanned
import android.text.StaticLayout
import android.text.TextPaint
import android.text.style.StyleSpan
import java.io.File

// Протокол встречи в PDF через PdfDocument: лист A4, системный шрифт (в нём есть кириллица),
// единица холста: пункт. Длинная реплика переносится на следующую страницу по строкам,
// заголовок реплики не остаётся внизу страницы без первой строки текста.
class ProtocolPdf(private val blocks: List<ProtocolBlock>) {
    private val doc = PdfDocument()
    private var page: PdfDocument.Page? = null
    private var pageCount = 0
    private var y = MARGIN

    private val body = paint(BODY_SIZE, Typeface.DEFAULT)
    private val bold = paint(BODY_SIZE, Typeface.DEFAULT_BOLD)
    private val title = paint(TITLE_SIZE, Typeface.DEFAULT_BOLD)
    private val border = Paint().apply {
        style = Paint.Style.STROKE
        strokeWidth = 0.5f
        color = Color.rgb(0xA0, 0xA0, 0xA0)
    }

    // Файл пишется целиком или не остаётся вовсе.
    fun writeTo(out: File) {
        try {
            newPage()
            for (block in blocks) draw(block)
            page?.let { doc.finishPage(it) }
            page = null
            out.outputStream().use { doc.writeTo(it) }
        } catch (e: Throwable) {
            out.delete()
            throw e
        } finally {
            doc.close()
        }
    }

    private fun draw(block: ProtocolBlock) {
        when (block) {
            is ProtocolBlock.Title -> {
                flow(layout(block.text, title, CONTENT_WIDTH))
                y += GAP
            }
            is ProtocolBlock.Fact -> flow(layout("${block.label}: ${block.value}", body, CONTENT_WIDTH))
            is ProtocolBlock.Table -> {
                y += GAP / 2
                tableRow(block.header, bold)
                for (row in block.rows) tableRow(row, body)
            }
            is ProtocolBlock.Turn -> turn(block)
        }
    }

    private fun turn(block: ProtocolBlock.Turn) {
        val heading = SpannableStringBuilder(block.speaker).apply {
            setSpan(StyleSpan(Typeface.BOLD), 0, length, Spanned.SPAN_EXCLUSIVE_EXCLUSIVE)
            append(", ${block.time}")
        }
        val head = layout(heading, body, CONTENT_WIDTH)
        val text = layout(block.text, body, CONTENT_WIDTH)
        val firstLine = if (text.lineCount > 0) text.getLineBottom(0).toFloat() else 0f
        if (!atTop()) y += GAP
        if (!atTop() && head.height + HEAD_GAP + firstLine > room()) newPage()
        flow(head)
        y += HEAD_GAP
        flow(text)
    }

    private fun tableRow(cells: List<String>, paint: TextPaint) {
        val widths = COLUMN_SHARES.map { (CONTENT_WIDTH * it).toInt() }
        val layouts = cells.mapIndexed { i, cell ->
            val width = widths.getOrElse(i) { widths[0] } - 2 * CELL_PAD.toInt()
            val align = if (i == 0) Layout.Alignment.ALIGN_NORMAL else Layout.Alignment.ALIGN_OPPOSITE
            layout(cell, paint, width, align)
        }
        val height = (layouts.maxOfOrNull { it.height } ?: 0) + 2 * CELL_PAD
        if (!atTop() && height > room()) newPage()
        val canvas = canvas()
        var x = MARGIN
        layouts.forEachIndexed { i, l ->
            val width = widths.getOrElse(i) { widths[0] }.toFloat()
            canvas.drawRect(x, y, x + width, y + height, border)
            canvas.save()
            canvas.translate(x + CELL_PAD, y + CELL_PAD)
            l.draw(canvas)
            canvas.restore()
            x += width
        }
        y += height
    }

    // Строки, которые не влезли, уходят на следующую страницу.
    private fun flow(l: StaticLayout) {
        var line = 0
        while (line < l.lineCount) {
            val top = l.getLineTop(line)
            if (!atTop() && l.getLineBottom(line) - top > room()) newPage()
            var last = line
            while (last + 1 < l.lineCount && l.getLineBottom(last + 1) - top <= room()) last++
            val height = (l.getLineBottom(last) - top).toFloat()
            val canvas = canvas()
            canvas.save()
            canvas.clipRect(MARGIN, y, MARGIN + l.width, y + height)
            canvas.translate(MARGIN, y - top)
            l.draw(canvas)
            canvas.restore()
            y += height
            line = last + 1
        }
    }

    private fun newPage() {
        page?.let { doc.finishPage(it) }
        pageCount += 1
        page = doc.startPage(PdfDocument.PageInfo.Builder(PAGE_WIDTH, PAGE_HEIGHT, pageCount).create())
        y = MARGIN
    }

    private fun canvas(): Canvas = page!!.canvas

    private fun atTop() = y <= MARGIN

    private fun room() = PAGE_HEIGHT - MARGIN - y

    private fun layout(
        text: CharSequence,
        paint: TextPaint,
        width: Int,
        align: Layout.Alignment = Layout.Alignment.ALIGN_NORMAL,
    ): StaticLayout = StaticLayout.Builder.obtain(text, 0, text.length, paint, width)
        .setAlignment(align)
        .setIncludePad(false)
        .build()

    private fun paint(size: Float, face: Typeface) = TextPaint(Paint.ANTI_ALIAS_FLAG).apply {
        textSize = size
        typeface = face
        color = Color.BLACK
    }

    companion object {
        // A4 в пунктах, поля 2 см.
        private const val PAGE_WIDTH = 595
        private const val PAGE_HEIGHT = 842
        private const val MARGIN = 57f
        private const val CONTENT_WIDTH = PAGE_WIDTH - 2 * 57
        private const val BODY_SIZE = 11f
        private const val TITLE_SIZE = 18f
        private const val GAP = 12f
        private const val HEAD_GAP = 3f
        private const val CELL_PAD = 4f
        private val COLUMN_SHARES = listOf(0.5f, 0.25f, 0.25f)
    }
}
