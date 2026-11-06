package dev.demo.nixyon.speech

// Границы кусков речи в сэмплах [начало, конец) с запасом pad с каждой стороны, как
// padded_bounds в движке на ноутбуке. VAD режет кусок вплотную к речи и часто отрезает
// начало первого слова или конец последнего, а модель на таком куске теряет слово
// целиком. Запас не заходит за середину промежутка до соседнего куска, чтобы одни и те
// же слова не распознавались дважды, и за края записи.
fun paddedBounds(bounds: List<Pair<Long, Long>>, total: Long, pad: Long): List<Pair<Long, Long>> =
    bounds.mapIndexed { i, (start, end) ->
        val leftLimit = if (i > 0) midpoint(bounds[i - 1].second, start) else 0L
        val rightLimit = if (i + 1 < bounds.size) midpoint(end, bounds[i + 1].first) else total
        val from = maxOf(start - pad, leftLimit).coerceAtMost(start).coerceAtLeast(0L)
        val to = minOf(end + pad, rightLimit).coerceAtLeast(end).coerceAtMost(total)
        from to maxOf(to, from)
    }

// Середина промежутка; если куски соприкасаются или перекрываются, граница остаётся
// там, где её поставил VAD.
private fun midpoint(leftEnd: Long, rightStart: Long): Long =
    if (rightStart <= leftEnd) rightStart else leftEnd + (rightStart - leftEnd) / 2
