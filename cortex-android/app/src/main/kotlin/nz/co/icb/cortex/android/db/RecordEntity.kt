package nz.co.icb.cortex.android.db

import androidx.room.Entity
import androidx.room.PrimaryKey

@Entity(tableName = "records")
data class RecordEntity(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    val tableName: String,
    val data: String,
    val createdAt: Long = System.currentTimeMillis()
)
