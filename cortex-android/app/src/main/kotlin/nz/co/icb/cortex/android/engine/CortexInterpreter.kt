package nz.co.icb.cortex.android.engine

import com.google.gson.Gson
import com.google.gson.JsonArray
import com.google.gson.JsonElement
import com.google.gson.JsonObject
import com.google.gson.JsonParser

class CortexInterpreter(private val nativeRegistry: AndroidNativeRegistry) {

    private val gson = Gson()
    private val variables = mutableMapOf<String, Any?>()
    private val logs = mutableListOf<String>()

    data class ExecutionResult(
        val logs: List<String>,
        val returnValue: Any? = null,
        val error: String? = null
    )

    fun execute(script: String, allow: String = "native.log,native.db,native.http,native.location"): ExecutionResult {
        logs.clear()
        variables.clear()
        return try {
            val astJson = CortexEngine.nativeCompile(script, allow)
            val ast = JsonParser.parseString(astJson).asJsonObject

            if (ast.has("error")) {
                return ExecutionResult(logs = emptyList(), error = ast.get("error").asString)
            }

            val result = walkNode(ast)
            ExecutionResult(logs = logs.toList(), returnValue = result)
        } catch (e: Exception) {
            ExecutionResult(logs = logs.toList(), error = e.message)
        }
    }

    private fun walkNode(node: JsonElement): Any? {
        if (!node.isJsonObject) return null
        val obj = node.asJsonObject
        val type = obj.get("type")?.asString ?: return null

        return when (type) {
            "Task" -> walkTask(obj)
            "Let" -> walkLet(obj)
            "If" -> walkIf(obj)
            "Return" -> walkReturn(obj)
            "Expr" -> walkNode(obj.get("expr"))
            "NativeCall" -> walkNativeCall(obj)
            "Binary" -> walkBinary(obj)
            "Unary" -> walkUnary(obj)
            "Literal" -> walkLiteral(obj)
            "Ident" -> walkIdent(obj)
            else -> null
        }
    }

    private fun walkTask(node: JsonObject): Any? {
        val body = node.get("body")?.asJsonArray ?: return null
        var result: Any? = null
        for (stmt in body) {
            result = walkNode(stmt)
            if (result is ReturnSignal) return result.value
        }
        return result
    }

    private fun walkLet(node: JsonObject): Any? {
        val name = node.get("name")?.asString ?: return null
        val value = walkNode(node.get("value"))
        variables[name] = value
        return value
    }

    private fun walkIf(node: JsonObject): Any? {
        val condition = walkNode(node.get("condition"))
        return if (isTruthy(condition)) {
            walkNode(node.get("then"))
        } else {
            node.get("else")?.let { walkNode(it) }
        }
    }

    private fun walkReturn(node: JsonObject): Any? {
        val value = walkNode(node.get("value"))
        return ReturnSignal(value)
    }

    private fun walkNativeCall(node: JsonObject): Any? {
        val api = node.get("api")?.asString ?: return null
        val args = node.get("args")?.asJsonArray?.map { walkNode(it) } ?: emptyList()
        return nativeRegistry.dispatch(api, args)
    }

    private fun walkBinary(node: JsonObject): Any? {
        val op = node.get("op")?.asString ?: return null
        val left = walkNode(node.get("left"))
        val right = walkNode(node.get("right"))
        return when (op) {
            "+" -> addValues(left, right)
            "-" -> toDouble(left) - toDouble(right)
            "*" -> toDouble(left) * toDouble(right)
            "/" -> toDouble(left) / toDouble(right)
            "==" -> left == right
            "!=" -> left != right
            "<" -> toDouble(left) < toDouble(right)
            ">" -> toDouble(left) > toDouble(right)
            "<=" -> toDouble(left) <= toDouble(right)
            ">=" -> toDouble(left) >= toDouble(right)
            "&&" -> isTruthy(left) && isTruthy(right)
            "||" -> isTruthy(left) || isTruthy(right)
            else -> null
        }
    }

    private fun walkUnary(node: JsonObject): Any? {
        val op = node.get("op")?.asString ?: return null
        val operand = walkNode(node.get("operand"))
        return when (op) {
            "!" -> !isTruthy(operand)
            "-" -> -toDouble(operand)
            else -> null
        }
    }

    private fun walkLiteral(node: JsonObject): Any? {
        val value = node.get("value") ?: return null
        return when {
            value.isJsonNull -> null
            value.isJsonPrimitive -> {
                val prim = value.asJsonPrimitive
                when {
                    prim.isBoolean -> prim.asBoolean
                    prim.isNumber -> prim.asDouble
                    prim.isString -> prim.asString
                    else -> null
                }
            }
            else -> value
        }
    }

    private fun walkIdent(node: JsonObject): Any? {
        val name = node.get("name")?.asString ?: return null
        return variables[name]
    }

    private fun isTruthy(value: Any?): Boolean = when (value) {
        null -> false
        is Boolean -> value
        is Double -> value != 0.0
        is String -> value.isNotEmpty()
        else -> true
    }

    private fun toDouble(value: Any?): Double = when (value) {
        is Double -> value
        is Boolean -> if (value) 1.0 else 0.0
        is String -> value.toDoubleOrNull() ?: 0.0
        else -> 0.0
    }

    private fun addValues(left: Any?, right: Any?): Any? {
        return if (left is String || right is String) {
            "${left}${right}"
        } else {
            toDouble(left) + toDouble(right)
        }
    }

    private data class ReturnSignal(val value: Any?)
}
